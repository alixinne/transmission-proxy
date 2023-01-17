use std::sync::Arc;

use async_session::{MemoryStore, Session, SessionStore};
use axum::{
    extract::Query,
    response::{IntoResponse, Redirect, Response},
    routing, Extension, Router,
};
use color_eyre::eyre;
use cookie::Cookie;
use hyper::{header::ACCEPT, StatusCode};
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use tower_cookies::Cookies;
use tracing::error;

use crate::server::auth::{UserClaim, COOKIE_NAME};

use super::Ctx;

pub(super) fn add_provider_routes(ctx: Arc<Ctx>, mut router: Router) -> eyre::Result<Router> {
    let bind = ctx.args.public_url();

    for provider in ctx.config.providers.oauth2.clone().into_iter() {
        // Skip disabled providers
        if !provider.enabled {
            continue;
        }

        // Clone
        let bind = bind.clone();

        // Parse json selector
        let selector = Arc::new(
            jsonpath::Selector::new(&provider.email_path).map_err(|err| {
                color_eyre::eyre::eyre!("invalid jsonpath for provider {}: {}", provider.name, err)
            })?,
        );

        // Memory store for this provider
        let ms = MemoryStore::new();

        // What we'll store in this session
        #[derive(Serialize, Deserialize)]
        struct AuthChallenge {
            pkce_verifier: PkceCodeVerifier,
            csrf_token: CsrfToken,
        }

        // What we get back
        #[derive(Deserialize)]
        struct CallbackQuery {
            state: CsrfToken,
            code: AuthorizationCode,
        }

        // Where we'll store the session id
        const SESSION_COOKIE_NAME: &str = "_transmission_proxy_session";

        // Create the oauth2 client
        let client = oauth2::basic::BasicClient::new(
            provider.client_id.clone(),
            Some(provider.client_secret.clone()),
            provider.auth_url.clone(),
            Some(provider.token_url.clone()),
        )
        .set_redirect_uri(
            oauth2::RedirectUrl::new(
                bind.to_string().trim_end_matches('/').to_owned()
                    + "/auth/"
                    + &provider.name
                    + "/callback",
            )
            .unwrap(),
        );

        router = router.nest(
            ("/auth/".to_owned() + &provider.name).as_str(),
            Router::new()
                .route(
                    "/login",
                    routing::get(
                        |Extension(client): Extension<oauth2::basic::BasicClient>,
                         cookies: Cookies,
                         Extension(store): Extension<MemoryStore>| async move {
                            let (pkce_challenge, pkce_verifier) =
                                PkceCodeChallenge::new_random_sha256();

                            let (auth_url, csrf_token) = {
                                let mut client = client.authorize_url(CsrfToken::new_random);

                                // Add scopes from provider config
                                for scope in
                                    provider.scopes.split(' ').filter(|scope| !scope.is_empty())
                                {
                                    client = client.add_scope(Scope::new(scope.into()));
                                }

                                client.set_pkce_challenge(pkce_challenge).url()
                            };

                            // Create session
                            let mut session = Session::new();
                            session
                                .insert(
                                    "challenge",
                                    AuthChallenge {
                                        pkce_verifier,
                                        csrf_token,
                                    },
                                )
                                .unwrap();

                            // Store session, set cookie
                            let cookie = store.store_session(session).await.unwrap().unwrap();
                            cookies.add(Cookie::build(SESSION_COOKIE_NAME, cookie).finish());

                            // Redirect to identity provider
                            Redirect::to(auth_url.as_str())
                        },
                    ),
                )
                .route(
                    "/callback",
                    routing::get(
                        move |Extension(ctx): Extension<Arc<Ctx>>,
                              Extension(client): Extension<oauth2::basic::BasicClient>,
                              cookies: Cookies,
                              Extension(store): Extension<MemoryStore>,
                              query: Query<CallbackQuery>| async move {
                            // Get the cookie
                            let session_cookie =
                                cookies.get(SESSION_COOKIE_NAME).ok_or_else(|| {
                                    (StatusCode::BAD_REQUEST, "Missing session cookie")
                                        .into_response()
                                })?;

                            // Get session
                            let session = store
                                .load_session(session_cookie.value().to_owned())
                                .await
                                .map_err(|_| {
                                    (StatusCode::BAD_REQUEST, "Invalid session cookie")
                                        .into_response()
                                })?
                                .ok_or_else(|| {
                                    (StatusCode::BAD_REQUEST, "Invalid session").into_response()
                                })?;

                            // Get challenge
                            let challenge: AuthChallenge = session.get("challenge").unwrap();

                            // Check state
                            if query.state.secret() != challenge.csrf_token.secret() {
                                return Err(
                                    (StatusCode::BAD_REQUEST, "Invalid CSRF Token").into_response()
                                );
                            }

                            // Fetch access token
                            let token_result = client
                                .exchange_code(query.code.clone())
                                .set_pkce_verifier(challenge.pkce_verifier)
                                .request_async(oauth2::reqwest::async_http_client)
                                .await
                                .map_err(|err| {
                                    error!(%err, "could not fetch oauth2 access token");
                                    (StatusCode::BAD_REQUEST, "Could not fetch token")
                                        .into_response()
                                })?;

                            // Get userinfo
                            let client = reqwest::Client::new();
                            let res = client
                                .get(provider.userinfo_url.as_ref())
                                .bearer_auth(token_result.access_token().secret())
                                .header(ACCEPT, "application/json")
                                .send()
                                .await
                                .map_err(|err| {
                                    error!(%err, "could not fetch userinfo");
                                    (StatusCode::BAD_GATEWAY, "Could not fetch userinfo")
                                        .into_response()
                                })?;

                            // Decode body
                            let body: serde_json::Value = res.json().await.map_err(|_err| {
                                (StatusCode::BAD_GATEWAY, "Could not parse userinfo body")
                                    .into_response()
                            })?;

                            // Get username
                            let username = selector
                                .find(&body)
                                .next()
                                .and_then(|value| value.as_str())
                                .ok_or_else(|| {
                                    error!(?body, "missing email in userinfo response");

                                    (
                                        StatusCode::BAD_GATEWAY,
                                        "Missing email in userinfo response",
                                    )
                                        .into_response()
                                })?
                                .to_string();

                            // Add claim to JWT
                            let claim = UserClaim::OAuth2 {
                                username,
                                provider: provider.name.clone(),
                            };

                            cookies.add(
                                Cookie::build(COOKIE_NAME, claim.jwt(&ctx.jwt_key))
                                    .same_site(cookie::SameSite::Strict)
                                    .http_only(true)
                                    .path(bind.path().to_string())
                                    .finish(),
                            );

                            // Redirect to application
                            Ok::<_, Response>(
                                Redirect::to(ctx.paths.web_path.as_str()).into_response(),
                            )
                        },
                    ),
                )
                .layer(Extension(client))
                .layer(Extension(ms)),
        );
    }

    Ok(router)
}
