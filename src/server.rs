use std::{convert::Infallible, sync::Arc};

use color_eyre::eyre;

use hmac::Mac;
use hyper::{
    header::LOCATION,
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server,
};
use tracing::{debug, info, span, warn, Instrument, Level};

use crate::{
    auth::AuthUser,
    config::Config,
    error::Error,
    ext::{ParsedRequest, RequestExt},
    rpc::proxy::RpcProxyClient,
    Args,
};

mod routes;
use routes::Routes;

mod views;
use views::Views;

pub type JwtKey = hmac::Hmac<sha2::Sha256>;

struct Ctx {
    args: Args,
    config: Config,
    client: RpcProxyClient,
    jwt_key: JwtKey,
    routes: Routes,
    views: Views,
}

impl Ctx {
    pub fn new(args: Args, config: Config) -> Self {
        let routes = Routes::new(&args);
        let views = Views::new();
        let jwt_key = JwtKey::new_from_slice(args.secret_key.as_bytes()).unwrap();

        let upstream = args.upstream.clone();
        Self {
            args,
            config,
            client: RpcProxyClient::new(upstream),
            jwt_key,
            routes,
            views,
        }
    }

    async fn handle_proxy_request(
        &self,
        req: Request<Body>,
        parsed: ParsedRequest,
    ) -> Result<Response<Body>, hyper::Error> {
        // Authenticate user
        let user = AuthUser::auth(&self.jwt_key, &parsed);

        // Check authorization
        let acl = self.config.acl.get(&user, &self.config.providers).await;

        if let Some(acl) = acl {
            // One ACL rule matched
            debug!(?acl, "matched acl");

            // Does this rule deny access?
            if acl.deny {
                if user.is_anonymous() {
                    // This is an unauthenticated user, redirect to the login page
                    return Ok(Response::builder()
                        .status(302)
                        .header(
                            LOCATION,
                            self.routes.login.path.clone()
                                + "?redirect_to="
                                + urlencoding::encode(&req.uri().to_string()).as_ref(),
                        )
                        .body(Body::empty())
                        .unwrap());
                } else {
                    // This is an authenticated, but not allowed user
                    return Ok(Response::builder()
                        .status(401)
                        .body(Body::from("Unauthorized"))
                        .unwrap());
                }
            }
        } else {
            // No ACL rules matched, authorize by default
            warn!(?acl, "no matched acl, running without authentication");
        }

        // Forward to upstream
        self.client.handle_request(req, acl).await
    }

    async fn handle_request(&self, req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
        let span = span!(
            Level::INFO,
            "request",
            method = ?req.method(),
            uri = ?req.uri(),
            headers = ?req.headers()
        );

        async move {
            match (req.method(), req.uri().path()) {
                (&Method::GET, "/healthz") => {
                    // Health check
                    Ok(Response::new(Body::empty()))
                }

                (_method, _path) => {
                    // Parse request data
                    let parsed = match req.parse() {
                        Ok(parsed) => parsed,
                        Err(err) => {
                            let response =
                                Response::builder().status(400).body(Body::empty()).unwrap();
                            info!(?response, %err);
                            return Ok(response);
                        }
                    };

                    if let Some(handler) = self.routes.handler(self, &req) {
                        handler.handle(self, req, parsed).await
                    } else {
                        let response = self.handle_proxy_request(req, parsed).await;
                        info!(?response);
                        response
                    }
                }
            }
        }
        .instrument(span)
        .await
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
    let ctx = Arc::new(Ctx::new(args, config));

    // Create hyper service fn
    let make_svc = make_service_fn(|_conn| {
        let ctx = ctx.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let ctx = ctx.clone();
                async move { ctx.handle_request(req).await }
            }))
        }
    });

    // Bind server
    let server = Server::try_bind(&addr)?
        .serve(make_svc)
        .instrument(server_span.clone());

    info!(parent: server_span, "listening");

    // Run server
    server.await?;

    Ok(())
}
