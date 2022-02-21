use std::{convert::Infallible, sync::Arc};

use color_eyre::eyre;

use hmac::Mac;
use hyper::{
    client::HttpConnector,
    header::{ACCEPT_ENCODING, CONTENT_LENGTH, HOST, LOCATION},
    service::{make_service_fn, service_fn},
    Body, Client, Method, Request, Response, Server, StatusCode, Uri,
};
use tracing::{debug, error, info, span, warn, Instrument, Level};

use crate::{
    acl::Acl,
    auth::AuthUser,
    config::Config,
    error::Error,
    ext::{ParsedRequest, RequestExt},
    rpc::{RpcMethodCall, RpcRequest, RpcResponse, RpcResponseKind, RpcResponseStatus},
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
    client: Client<HttpConnector, Body>,
    jwt_key: JwtKey,
    routes: Routes,
    views: Views,
}

impl Ctx {
    pub fn new(args: Args, config: Config) -> Self {
        let routes = Routes::new(&args);
        let views = Views::new();
        let jwt_key = JwtKey::new_from_slice(args.secret_key.as_bytes()).unwrap();

        Self {
            args,
            config,
            client: Client::new(),
            jwt_key,
            routes,
            views,
        }
    }

    fn get_upstream_url(&self, req_url: &Uri) -> Uri {
        let mut parts = self.args.upstream.clone().into_parts();

        // TODO: Combine upstream path instead of replacing
        parts.path_and_query = req_url.path_and_query().cloned();

        // TODO: Handle possible errors
        Uri::from_parts(parts).expect("failed building upstream uri")
    }

    fn rpc_failure<T>(
        &self,
        err: impl std::fmt::Display,
        status: T,
        tag: Option<i32>,
    ) -> Response<Body>
    where
        StatusCode: TryFrom<T>,
        <StatusCode as TryFrom<T>>::Error: Into<hyper::http::Error>,
    {
        Response::builder()
            .status(status)
            .body(Body::from(
                serde_json::to_string(&RpcResponse {
                    tag,
                    arguments: None,
                    result: RpcResponseStatus::Failure(err.to_string()),
                })
                .unwrap(),
            ))
            .unwrap()
    }

    async fn handle_rpc_request(
        &self,
        mut req: Request<Body>,
        acl: Option<&Acl>,
    ) -> Result<Response<Body>, hyper::Error> {
        // We don't accept gzip to simplify things for rpc mapping
        req.headers_mut().remove(ACCEPT_ENCODING);

        // Parse the request body
        let req_body_bytes = hyper::body::to_bytes(req.body_mut()).await?;
        *req.body_mut() = Body::from(req_body_bytes.clone());

        match serde_json::from_slice::<RpcRequest>(&req_body_bytes) {
            Ok(mut rpc_request) => {
                // Check ACL
                if let Some(acl) = acl {
                    if !acl.allowed_methods.is_empty() {
                        // Restrict allowed methods
                        if !acl.allowed_methods.contains(&(&rpc_request.call).into()) {
                            return Ok(self.rpc_failure("forbidden", 403, rpc_request.tag));
                        }
                    }
                }

                // Check that torrent add respects the download dir
                // TODO: Handle TorrentSet
                // TODO: Handle TorrentSetLocation
                // TODO: Handle TorrentRenamePath
                // TODO: Check that torrents are authorized based on download_dir
                match &mut rpc_request.call {
                    RpcMethodCall::TorrentAdd { arguments } => {
                        if let Some(download_dir) = acl.and_then(|acl| acl.download_dir.as_ref()) {
                            if download_dir != &arguments.download_dir {
                                // The download dir field was tampered with
                                return Ok(self.rpc_failure("forbidden", 403, rpc_request.tag));
                            }
                        }

                        if let Some(tracker_rules) = acl.and_then(|acl| {
                            (!acl.tracker_rules.is_empty()).then(|| &acl.tracker_rules)
                        }) {
                            // Parse torrent in metainfo
                            if !arguments.metainfo.is_empty() {
                                if let Some(mut torrent) = base64::decode(&arguments.metainfo)
                                    .ok()
                                    .as_ref()
                                    .and_then(|bencoded| {
                                        serde_bencode::de::from_bytes::<crate::torrent::Torrent>(
                                            bencoded,
                                        )
                                        .ok()
                                    })
                                {
                                    // Replace announce list
                                    if torrent
                                        .announce_list
                                        .as_ref()
                                        .map(|list| !list.is_empty())
                                        .unwrap_or(false)
                                    {
                                        // TODO: Support announce list
                                        return Ok(self.rpc_failure(
                                            "not implemented",
                                            501,
                                            rpc_request.tag,
                                        ));
                                    }

                                    // Replace main announce URL
                                    if let Some(announce) = &mut torrent.announce {
                                        for rule in tracker_rules.iter() {
                                            if !rule.matches(announce.as_str()) {
                                                continue;
                                            }

                                            if let Some(result) = rule.apply(announce.as_str()) {
                                                *announce = result;
                                            } else {
                                                // The announce URL was removed
                                                torrent.announce = None;
                                                break;
                                            }
                                        }
                                    }

                                    // Replace argument
                                    if let Some(metainfo) = serde_bencode::ser::to_bytes(&torrent)
                                        .ok()
                                        .map(base64::encode)
                                    {
                                        arguments.metainfo = metainfo;
                                    } else {
                                        // TODO: Report error
                                        warn!("error encoding torrent");
                                        return Ok(self.rpc_failure(
                                            "internal server error",
                                            500,
                                            rpc_request.tag,
                                        ));
                                    }
                                } else {
                                    // TODO: Report error
                                    warn!("error parsing torrent");
                                    return Ok(self.rpc_failure(
                                        "bad request",
                                        400,
                                        rpc_request.tag,
                                    ));
                                }
                            } else {
                                // TODO: Support magnet links
                                return Ok(self.rpc_failure(
                                    "not implemented",
                                    501,
                                    rpc_request.tag,
                                ));
                            }
                        }
                    }
                    _ => {}
                }

                // Replace body
                *req.body_mut() = Body::from(serde_json::to_string(&rpc_request).unwrap());
                req.headers_mut().remove(CONTENT_LENGTH);
            }

            Err(err) => {
                warn!(%err, body = %String::from_utf8_lossy(&req_body_bytes), "could not parse request body");

                return Ok(self.rpc_failure(err, 400, None));
            }
        }

        // Fetch response
        let mut response = self.client.request(req).await?;
        debug!(?response);

        // Decode the response body
        let mut bytes = hyper::body::to_bytes(response.body_mut()).await?.to_vec();

        // Perform replacements in RPC response
        if let Some::<RpcResponse>(mut rpc_response) = serde_json::from_slice(&bytes)
            .map_err(|err| {
                error!(?err);
            })
            .ok()
        {
            if let Some(download_dir) = acl.and_then(|acl| acl.download_dir.as_ref()) {
                match &mut rpc_response.arguments {
                    Some(RpcResponseKind::Config(config)) => {
                        config.download_dir = download_dir.to_owned();
                    }

                    Some(RpcResponseKind::Torrents { torrents, .. }) => {
                        *torrents = torrents
                            .drain(..)
                            .filter(|torrent| {
                                // Strip trailing /
                                let torrent_download_dir = torrent
                                    .download_dir
                                    .strip_suffix('/')
                                    .unwrap_or(torrent.download_dir.as_str());

                                torrent_download_dir.starts_with(download_dir)
                            })
                            .collect();
                    }

                    _ => {}
                }

                bytes = serde_json::to_string(&rpc_response)
                    .expect("failed to serialize response")
                    .into();
            }
        }

        // Replace response body and return response
        let (mut parts, _) = response.into_parts();
        parts.headers.remove(CONTENT_LENGTH);
        Ok(Response::from_parts(parts, Body::from(bytes)))
    }

    async fn handle_other_request(
        &self,
        req: Request<Body>,
    ) -> Result<Response<Body>, hyper::Error> {
        Ok(self.client.request(req).await?)
    }

    async fn handle_proxy_request(
        &self,
        mut req: Request<Body>,
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

        // Update target url
        *req.uri_mut() = self.get_upstream_url(req.uri());
        req.headers_mut().remove(HOST);

        // Forward to upstream
        if req.uri().path().ends_with("/rpc") {
            self.handle_rpc_request(req, acl).await
        } else {
            self.handle_other_request(req).await
        }
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
