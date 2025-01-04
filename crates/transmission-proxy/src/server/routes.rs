use std::sync::Arc;

use axum::{
    extract::Query,
    response::{IntoResponse, Redirect},
    Extension,
};
use cookie::{time::OffsetDateTime, Cookie};
use hyper::{
    header::{USER_AGENT, WWW_AUTHENTICATE},
    Body, Request, Response,
};
use serde::{Deserialize, Serialize};
use tower_cookies::Cookies;
use tracing::{debug, warn};

use crate::{auth::AuthUser, Args};

use super::{
    auth::{UserClaim, COOKIE_NAME},
    views, Ctx,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRedirect {
    redirect_to: Option<String>,
}

pub struct Paths {
    pub login_path: String,
    pub web_path: String,
}

impl Paths {
    pub fn new(args: &Args) -> Self {
        let base = args.bind.path().trim_end_matches('/');

        Self {
            login_path: base.to_owned() + "/login",
            web_path: base.to_owned() + "/web/",
        }
    }
}

pub(super) async fn default(Extension(ctx): Extension<Arc<Ctx>>) -> impl IntoResponse {
    let url = ctx.paths.login_path.clone()
        + "?redirect_to="
        + urlencoding::encode(&ctx.paths.web_path).as_ref();
    debug!(%url, "Redirecting to login page by default");
    Redirect::to(&url)
}

pub(super) async fn healthz() {
    // empty
}

pub(super) async fn login(
    Extension(ctx): Extension<Arc<Ctx>>,
    query: Query<AuthRedirect>,
    user: AuthUser,
) -> impl IntoResponse {
    if user.is_anonymous() {
        ctx.views
            .render(&views::login::Data {
                config: &ctx.config,
                redirect_to: query.redirect_to.clone(),
            })
            .unwrap()
            .into_response()
    } else {
        // Authenticated, redirect
        let url = query
            .redirect_to
            .as_deref()
            .unwrap_or(ctx.paths.web_path.as_str());
        debug!(%url, "Redirecting authenticated user");
        Redirect::to(url).into_response()
    }
}

pub(super) async fn logout(
    Extension(ctx): Extension<Arc<Ctx>>,
    cookies: Cookies,
) -> impl IntoResponse {
    cookies.add(
        Cookie::build(COOKIE_NAME, "")
            .expires(OffsetDateTime::now_utc() - cookie::time::Duration::new(60, 0))
            .finish(),
    );

    let url = &ctx.paths.login_path;
    debug!(%url, "Redirecting user after logout");
    Redirect::to(url)
}

pub(super) async fn auth_basic(
    Extension(ctx): Extension<Arc<Ctx>>,
    query: Query<AuthRedirect>,
    cookies: Cookies,
    user: AuthUser,
) -> impl IntoResponse {
    if user.is_anonymous() {
        // Not authenticated
        Response::builder()
            .status(401)
            .header(
                WWW_AUTHENTICATE,
                r#"Basic realm="Transmission", charset="UTF-8""#.to_owned(),
            )
            .body(Body::empty())
            .unwrap()
            .into_response()
    } else {
        // Authenticated, redirect
        cookies.add(
            Cookie::build(
                COOKIE_NAME,
                UserClaim::from_auth_user(&user).unwrap().jwt(&ctx.jwt_key),
            )
            .same_site(cookie::SameSite::Strict)
            .http_only(true)
            .path(ctx.args.public_url().path().to_string())
            .finish(),
        );

        let url = query
            .redirect_to
            .as_deref()
            .unwrap_or(ctx.paths.web_path.as_str());
        debug!(%url, "Redirecting user after authentication");
        Redirect::to(url).into_response()
    }
}

pub(super) async fn proxy_request(
    Extension(ctx): Extension<Arc<Ctx>>,
    user: AuthUser,
    req: Request<Body>,
) -> impl IntoResponse {
    // Check authorization
    let acl = ctx.config.acl.get(&user, &ctx.config.providers).await;

    if let Some(acl) = acl {
        // One ACL rule matched
        debug!(?acl, ?user, "matched acl");

        // Does this rule deny access?
        if acl.deny {
            if user.is_anonymous() {
                if req.headers().get(USER_AGENT).map(|hdr| hdr.as_ref())
                    == Some(b"transmission-remote-gtk")
                {
                    // Unauthenticated client app, this will always use basic auth
                    return Response::builder()
                        .status(401)
                        .header(WWW_AUTHENTICATE, "Basic realm=\"Transmission\"")
                        .body(Body::empty())
                        .unwrap()
                        .into_response();
                }

                // This is an unauthenticated user, redirect to the login page
                let url = ctx.paths.login_path.clone()
                    + "?redirect_to="
                    + urlencoding::encode(&req.uri().to_string()).as_ref();
                debug!(%url, "Redirecting unauthenticated user");
                return Redirect::to(&url).into_response();
            } else {
                // This is an authenticated, but not allowed user
                return Response::builder()
                    .status(401)
                    .body(Body::from("Unauthorized"))
                    .unwrap()
                    .into_response();
            }
        }
    } else {
        // No ACL rules matched, authorize by default
        warn!(
            ?acl,
            ?user,
            "no matched acl, running without authentication"
        );
    }

    // Forward to upstream
    match ctx.client.handle_request(req, acl).await {
        Ok(response) => response.into_response(),
        Err(err) => Response::builder()
            .status(500)
            .body(Body::from(err.to_string()))
            .unwrap()
            .into_response(),
    }
}
