use std::future::Future;
use std::pin::Pin;

use cookie::{time::OffsetDateTime, Cookie};
use hyper::{
    header::{LOCATION, SET_COOKIE, WWW_AUTHENTICATE},
    Body, Method, Request, Response,
};

use crate::{
    auth::{AuthUser, COOKIE_NAME},
    ext::ParsedRequest,
    Args,
};

use super::{views, Ctx};

pub struct RouteHandler {
    pub path: String,
    handle_fn: Box<
        dyn for<'c> Fn(
                &'c Ctx,
                Request<Body>,
                ParsedRequest,
            ) -> Pin<
                Box<dyn Future<Output = Result<Response<Body>, hyper::Error>> + Send + 'c>,
            > + Send
            + Sync,
    >,
}

impl RouteHandler {
    fn new<H>(path: String, handler: H) -> Self
    where
        H: Sync
            + Send
            + for<'c> Fn(
                &'c Ctx,
                Request<Body>,
                ParsedRequest,
            ) -> Pin<
                Box<dyn Future<Output = Result<Response<Body>, hyper::Error>> + Send + 'c>,
            > + 'static,
    {
        Self {
            path,
            handle_fn: Box::new(handler),
        }
    }

    pub(super) async fn handle(
        &self,
        ctx: &Ctx,
        req: Request<Body>,
        parsed: ParsedRequest,
    ) -> Result<Response<Body>, hyper::Error> {
        (*self.handle_fn)(ctx, req, parsed).await
    }
}

pub struct Routes {
    pub login: RouteHandler,
    pub logout: RouteHandler,
    pub auth_basic: RouteHandler,
    pub web_path: String,
}

impl Routes {
    fn route_path(base: &str, path: &str) -> String {
        if base.ends_with('/') {
            base.to_string() + &path[1..]
        } else {
            base.to_string() + path
        }
    }

    pub fn new(args: &Args) -> Self {
        let login_path = Self::route_path(args.bind.path(), "/login");
        let logout_path = Self::route_path(args.bind.path(), "/logout");
        let auth_basic_path = Self::route_path(args.bind.path(), "/auth/basic");
        let web_path = Self::route_path(args.bind.path(), "/web/");

        Self {
            login: RouteHandler::new(login_path, move |ctx, _req, parsed| {
                Box::pin(async move {
                    Ok(ctx
                        .views
                        .render(&views::login::Data {
                            config: &ctx.config,
                            redirect_to: parsed
                                .query_parameters
                                .get("redirect_to")
                                .map(String::to_string),
                        })
                        .unwrap())
                })
            }),
            logout: RouteHandler::new(logout_path, move |ctx, _req, _parsed| {
                Box::pin(async move {
                    // This is an unauthenticated user, redirect to the login page
                    Ok(Response::builder()
                        .status(302)
                        .header(LOCATION, ctx.routes.login.path.clone())
                        .header(
                            SET_COOKIE,
                            Cookie::build(COOKIE_NAME, "")
                                .expires(
                                    OffsetDateTime::now_utc() - cookie::time::Duration::new(60, 0),
                                )
                                .finish()
                                .encoded()
                                .to_string(),
                        )
                        .body(Body::empty())
                        .unwrap())
                })
            }),
            auth_basic: RouteHandler::new(auth_basic_path, move |ctx, _req, parsed| {
                Box::pin(async move {
                    // Check if the user is currently authenticated
                    let user = AuthUser::auth(&ctx.jwt_key, &parsed);

                    if user.is_anonymous() {
                        // Not authenticated
                        Ok(Response::builder()
                            .status(401)
                            .header(
                                WWW_AUTHENTICATE,
                                r#"Basic realm="Transmission", charset="UTF-8""#.to_owned(),
                            )
                            .body(Body::empty())
                            .unwrap())
                    } else {
                        // Authenticated, redirect
                        let redirect_to = parsed
                            .query_parameters
                            .get("redirect_to")
                            .map(String::to_string)
                            .unwrap_or_else(|| ctx.routes.web_path.clone());

                        Ok(Response::builder()
                            .status(302)
                            .header(LOCATION, redirect_to)
                            .header(
                                SET_COOKIE,
                                Cookie::build(COOKIE_NAME, user.claim().unwrap().jwt(&ctx.jwt_key))
                                    .same_site(cookie::SameSite::Strict)
                                    .http_only(true)
                                    .path(ctx.args.bind.path())
                                    .finish()
                                    .encoded()
                                    .to_string(),
                            )
                            .body(Body::empty())
                            .unwrap())
                    }
                })
            }),
            web_path,
        }
    }

    pub(super) fn handler(&self, ctx: &Ctx, req: &Request<Body>) -> Option<&RouteHandler> {
        if req.method() == Method::GET {
            let path = req.uri().path();

            if path == self.login.path {
                return Some(&self.login);
            } else if path == self.logout.path {
                return Some(&self.logout);
            } else if ctx.config.providers.basic.enabled && path == self.auth_basic.path {
                return Some(&self.auth_basic);
            }
        }

        None
    }
}
