use std::sync::Arc;

use axum::{routing, Extension, Router};
use color_eyre::eyre;

use hmac::Mac;
use hyper::Server;

use tower_cookies::CookieManagerLayer;
use tracing::{info, span, Instrument, Level};

use crate::{config::Config, error::Error, rpc::proxy::RpcProxyClient, Args};

mod auth;
mod oauth;
mod routes;
mod views;
use views::Views;

use self::routes::Paths;

pub type JwtKey = hmac::Hmac<sha2::Sha256>;

struct Ctx {
    args: Args,
    config: Config,
    client: RpcProxyClient,
    jwt_key: JwtKey,
    views: Views,
    paths: Paths,
}

impl Ctx {
    pub fn new(args: Args, config: Config) -> Self {
        let views = Views::new();
        let jwt_key = JwtKey::new_from_slice(args.secret_key.as_bytes()).unwrap();
        let paths = Paths::new(&args);

        let upstream = args.upstream.clone();
        Self {
            args,
            config,
            client: RpcProxyClient::new(upstream),
            jwt_key,
            views,
            paths,
        }
    }
}

pub async fn run(args: Args, config: Config) -> eyre::Result<()> {
    // Server status span
    let server_span = span!(Level::INFO, "server", addr = %args.bind);

    // Resolve bind addr
    let addr = {
        let _guard = server_span.enter();

        // TODO: Reduce allocations here
        tokio::net::lookup_host(
            (args.bind.host().unwrap_or("localhost").to_string() + ":")
                + args
                    .bind
                    .port()
                    .map(|port| port.as_str().to_string())
                    .unwrap_or_else(|| "80".to_owned())
                    .as_str(),
        )
        .await?
        .next()
        .ok_or_else(|| Error::BindResolve(args.bind.clone()))?
    };

    // Initialize context
    let bind = args.bind.clone();
    let ctx = Arc::new(Ctx::new(args, config));

    // Create axum router
    // Nested routes
    let sub_router = {
        let router = Router::new()
            .route("/", routing::get(routes::default))
            .route("/login", routing::get(routes::login))
            .route("/logout", routing::get(routes::logout));

        // Enable basic auth
        let router = if ctx.config.providers.basic.enabled {
            router.route("/auth/basic", routing::get(routes::auth_basic))
        } else {
            router
        };

        // Enable oauth routes
        oauth::add_provider_routes(ctx.clone(), router)?
    };

    // Root routes
    let router = Router::new()
        .route("/", routing::get(routes::default))
        .route("/healthz", routing::get(routes::healthz))
        .nest(bind.path(), sub_router)
        .fallback(routing::get(routes::proxy_request).post(routes::proxy_request))
        .layer(Extension(ctx.clone()))
        .layer(CookieManagerLayer::new());

    // Bind server
    let server = Server::try_bind(&addr)?
        .serve(router.into_make_service())
        .instrument(server_span.clone());

    info!(parent: server_span, "listening");

    // Run server
    server.await?;

    Ok(())
}
