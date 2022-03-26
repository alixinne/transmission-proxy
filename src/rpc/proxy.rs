use hyper::{
    client::HttpConnector,
    header::{ACCEPT_ENCODING, CONTENT_LENGTH, HOST},
    Body, Client, Uri,
};
use thiserror::Error;
use tracing::{debug, error, warn};

use crate::{
    acl::{Acl, TrackerRule},
    rpc::RawResponse,
};

use super::{
    MethodCall, Request, Response, ResponseKind, ResponseStatus, SessionArguments, TorrentAction,
    TorrentGet, TorrentIds, TorrentRemove, TorrentRenamePath, TorrentSet, TorrentSetLocation,
    Torrents,
};

/// Trait for requests that hold torrent ids
trait HasTorrentIds: Send + Sync {
    /// true if the response torrents should be filtered instead of the request
    fn filters_on_response(&self) -> bool {
        false
    }

    /// Get the torrent ids
    fn ids(&self) -> &Option<TorrentIds>;
    /// Get a mutable reference to the torrent ids
    fn ids_mut(&mut self) -> &mut Option<TorrentIds>;
}

macro_rules! impl_has_torrent_ids {
    ($t:ty) => {
        impl HasTorrentIds for $t {
            fn ids(&self) -> &Option<TorrentIds> {
                &self.ids
            }

            fn ids_mut(&mut self) -> &mut Option<TorrentIds> {
                &mut self.ids
            }
        }
    };

    ($t:ty, $e:expr) => {
        impl HasTorrentIds for $t {
            fn filters_on_response(&self) -> bool {
                $e
            }

            fn ids(&self) -> &Option<TorrentIds> {
                &self.ids
            }

            fn ids_mut(&mut self) -> &mut Option<TorrentIds> {
                &mut self.ids
            }
        }
    };
}

impl_has_torrent_ids!(TorrentAction);
impl_has_torrent_ids!(TorrentSet);
impl_has_torrent_ids!(TorrentGet, true);
impl_has_torrent_ids!(TorrentRemove);
impl_has_torrent_ids!(TorrentSetLocation);
impl_has_torrent_ids!(TorrentRenamePath);

trait MaybeTorrentIds {
    fn torrent_ids(&self) -> Option<&dyn HasTorrentIds>;
    fn torrent_ids_mut(&mut self) -> Option<&mut dyn HasTorrentIds>;
}

impl MaybeTorrentIds for MethodCall {
    fn torrent_ids(&self) -> Option<&dyn HasTorrentIds> {
        match self {
            MethodCall::TorrentStart { arguments } => Some(arguments),
            MethodCall::TorrentStartNow { arguments } => Some(arguments),
            MethodCall::TorrentStop { arguments } => Some(arguments),
            MethodCall::TorrentVerify { arguments } => Some(arguments),
            MethodCall::TorrentReannounce { arguments } => Some(arguments),
            MethodCall::TorrentSet { arguments } => Some(arguments),
            MethodCall::TorrentGet { arguments } => Some(arguments),
            MethodCall::TorrentRemove { arguments } => Some(arguments),
            MethodCall::TorrentSetLocation { arguments } => Some(arguments),
            MethodCall::TorrentRenamePath { arguments } => Some(arguments),
            _ => None,
        }
    }

    fn torrent_ids_mut(&mut self) -> Option<&mut dyn HasTorrentIds> {
        match self {
            MethodCall::TorrentStart { arguments } => Some(arguments),
            MethodCall::TorrentStartNow { arguments } => Some(arguments),
            MethodCall::TorrentStop { arguments } => Some(arguments),
            MethodCall::TorrentVerify { arguments } => Some(arguments),
            MethodCall::TorrentReannounce { arguments } => Some(arguments),
            MethodCall::TorrentSet { arguments } => Some(arguments),
            MethodCall::TorrentGet { arguments } => Some(arguments),
            MethodCall::TorrentRemove { arguments } => Some(arguments),
            MethodCall::TorrentSetLocation { arguments } => Some(arguments),
            MethodCall::TorrentRenamePath { arguments } => Some(arguments),
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
pub struct FilterError {
    pub tag: Option<i32>,
    pub kind: FilterErrorKind,
}

impl std::fmt::Display for FilterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.kind.fmt(f)
    }
}

#[derive(Debug, Error)]
pub enum FilterErrorKind {
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
    #[error("access denied")]
    Forbidden,
    #[error("torrent error")]
    Torrent(#[from] serde_bencode::Error),
    #[error("base64 error")]
    Base64(#[from] base64::DecodeError),
    #[error("could not parse request body")]
    ParseBody,
    #[error("could not decode body")]
    Serde(#[from] serde_json::Error),
    #[error("upstream error")]
    Upstream(#[from] hyper::Error),
    #[error("unknown upstream error")]
    UpstreamUnknown,
}

impl From<FilterError> for hyper::Response<hyper::Body> {
    fn from(value: FilterError) -> Self {
        hyper::Response::builder()
            .status(match value.kind {
                FilterErrorKind::Unsupported(_) => 501,
                FilterErrorKind::Forbidden => 403,
                FilterErrorKind::Torrent(_)
                | FilterErrorKind::Base64(_)
                | FilterErrorKind::ParseBody => 400,
                FilterErrorKind::Serde(_) => 500,
                FilterErrorKind::Upstream(_) => 503,
                FilterErrorKind::UpstreamUnknown => 502,
            })
            .body(hyper::Body::from(
                serde_json::to_string(&Response {
                    tag: value.tag,
                    arguments: None,
                    result: ResponseStatus::Failure(value.kind.to_string()),
                })
                .unwrap(),
            ))
            .unwrap()
    }
}

pub struct RpcProxyClient {
    upstream: Uri,
    client: Client<HttpConnector, Body>,
}

impl RpcProxyClient {
    pub fn new(upstream: Uri) -> Self {
        Self {
            upstream,
            client: Client::new(),
        }
    }

    async fn filter_torrent_ids(
        &self,
        torrent_ids: &mut dyn HasTorrentIds,
        current_rpc_request: &hyper::Request<Body>,
    ) -> Result<(), FilterErrorKind> {
        if torrent_ids.filters_on_response() {
            // Nothing to do, filter on response
            Ok(())
        } else {
            let input = torrent_ids.ids().clone();

            // Fetch torrent details so we can authorize the targets
            let torrents_rpc_req = Request {
                call: MethodCall::TorrentGet {
                    arguments: TorrentGet {
                        ids: input.clone(),
                        fields: vec!["id".to_owned(), "downloadDir".to_owned()],
                        format: Default::default(),
                    },
                },
                tag: None,
            };

            // Prepare the HTTP request
            let mut req = hyper::Request::builder()
                .uri(current_rpc_request.uri())
                .method(current_rpc_request.method())
                .body(Body::from(
                    serde_json::to_string(&torrents_rpc_req).unwrap(),
                ))
                .unwrap();

            for header in current_rpc_request.headers() {
                if header.0 != CONTENT_LENGTH {
                    req.headers_mut().insert(header.0, header.1.clone());
                }
            }

            // Send it
            let mut res = self.client.request(req).await?;

            // Decode the response
            let response: RawResponse =
                serde_json::from_slice(hyper::body::to_bytes(res.body_mut()).await?.as_ref())?;

            // Decode the response arguments
            let torrents: Torrents = serde_json::from_value(
                response
                    .arguments
                    .ok_or_else(|| FilterErrorKind::UpstreamUnknown)?,
            )?;

            *torrent_ids.ids_mut() = Some(TorrentIds::Ids(
                torrents
                    .torrents
                    .into_iter()
                    .map(|torrent| torrent.id)
                    .collect(),
            ));

            debug!(input = ?input, output = ?torrent_ids.ids().as_ref().unwrap(), "filtered torrent ids");

            Ok(())
        }
    }

    fn filter_tracker(&self, tracker: &mut Option<String>, tracker_rules: &[TrackerRule]) {
        for rule in tracker_rules.iter() {
            if let Some(announce) = tracker {
                if !rule.matches(announce.as_str()) {
                    continue;
                }

                if let Some(result) = rule.apply(announce.as_str()) {
                    *tracker = Some(result);
                } else {
                    // The announce URL was removed
                    *tracker = None;
                }
            } else {
                break;
            }
        }
    }

    fn filter_tracker_list(&self, tracker_list: &mut Vec<String>, tracker_rules: &[TrackerRule]) {
        let mut new_list = Vec::with_capacity(tracker_list.len());

        for item in tracker_list.iter() {
            let mut result = Some(item.clone());

            self.filter_tracker(&mut result, tracker_rules);

            if let Some(announce) = result {
                new_list.push(announce);
            }
        }

        *tracker_list = new_list;
    }

    fn prefix_ok(&self, location: &str, acl: &Acl) -> bool {
        if let Some(download_dir) = &acl.download_dir {
            let prefix = if download_dir.ends_with("/") {
                download_dir.clone()
            } else {
                format!("{}/", download_dir)
            };

            if !location.starts_with(&prefix) {
                // The download dir field was tampered with
                return false;
            }
        }

        true
    }

    async fn do_filter_request(
        &self,
        mut request: Request,
        acl: &Acl,
        current_rpc_request: &hyper::Request<Body>,
    ) -> Result<Request, FilterErrorKind> {
        // Check ACL
        if !acl.allowed_methods.is_empty() {
            // Restrict allowed methods
            if !acl.allowed_methods.contains(&(&request.call).into()) {
                return Err(FilterErrorKind::Forbidden);
            }
        }

        // Filter torrent ids
        if acl.download_dir.is_some() {
            if let Some(torrent_ids) = request.call.torrent_ids_mut() {
                self.filter_torrent_ids(torrent_ids, current_rpc_request)
                    .await
                    .map_err(|err| {
                        error!(?err, "failed filtering torrent ids");
                        err
                    })?;
            }
        }

        match &mut request.call {
            // Torrent actions: they were authorized by filter_torrent_ids
            MethodCall::TorrentStart { .. } => Ok(request),
            MethodCall::TorrentStartNow { .. } => Ok(request),
            MethodCall::TorrentStop { .. } => Ok(request),
            MethodCall::TorrentVerify { .. } => Ok(request),
            MethodCall::TorrentReannounce { .. } => Ok(request),
            MethodCall::TorrentRemove { .. } => Ok(request),

            // Queue actions: they were authorized by filter_torrent_ids
            MethodCall::QueueMoveTop { .. } => Ok(request),
            MethodCall::QueueMoveUp { .. } => Ok(request),
            MethodCall::QueueMoveDown { .. } => Ok(request),
            MethodCall::QueueMoveBottom { .. } => Ok(request),

            MethodCall::TorrentSet { arguments } => {
                // Check the new location, if any
                if let Some(new_location) = &arguments.location {
                    if !self.prefix_ok(new_location, acl) {
                        return Err(FilterErrorKind::Forbidden);
                    }
                }

                if let Some(tracker_rules) =
                    (!acl.tracker_rules.is_empty()).then(|| &acl.tracker_rules)
                {
                    self.filter_tracker_list(&mut arguments.tracker_add, tracker_rules);
                    self.filter_tracker_list(&mut arguments.tracker_remove, tracker_rules);

                    // TODO: Support trackerReplace
                    if !arguments.tracker_replace.is_empty() {
                        return Err(FilterErrorKind::Unsupported(
                            "trackerReplace in torrent-set",
                        ));
                    }
                }

                Ok(request)
            }

            MethodCall::TorrentSetLocation { arguments } => {
                if !self.prefix_ok(&arguments.location, acl) {
                    return Err(FilterErrorKind::Forbidden);
                }

                Ok(request)
            }

            // TODO: Could this be used to escape path filtering?
            MethodCall::TorrentRenamePath { .. } => Ok(request),

            // Session methods: authorized by acl.allowed_methods
            MethodCall::SessionSet { .. } => Ok(request),
            MethodCall::SessionGet { .. } => Ok(request),
            MethodCall::SessionStats => Ok(request),
            MethodCall::BlocklistUpdate => Ok(request),
            MethodCall::PortTest => Ok(request),
            MethodCall::SessionClose => Ok(request),
            MethodCall::FreeSpace { .. } => Ok(request),

            // Torrent get: filters on response
            MethodCall::TorrentGet { .. } => Ok(request),

            MethodCall::TorrentAdd { arguments } => {
                if !self.prefix_ok(&arguments.download_dir, acl) {
                    return Err(FilterErrorKind::Forbidden);
                }

                if let Some(tracker_rules) =
                    (!acl.tracker_rules.is_empty()).then(|| &acl.tracker_rules)
                {
                    // Parse torrent in metainfo
                    if !arguments.metainfo.is_empty() {
                        let mut torrent = serde_bencode::de::from_bytes::<crate::torrent::Torrent>(
                            base64::decode(&arguments.metainfo)?.as_ref(),
                        )?;

                        // Replace announce list
                        for list in &mut torrent.announce_list {
                            for sublist in list.iter_mut() {
                                self.filter_tracker_list(sublist, tracker_rules);
                            }
                        }

                        // Replace main announce URL
                        self.filter_tracker(&mut torrent.announce, tracker_rules);

                        // Replace argument
                        arguments.metainfo =
                            base64::encode(serde_bencode::ser::to_bytes(&torrent)?);

                        Ok(request)
                    } else {
                        // TODO: Support magnet links
                        Err(FilterErrorKind::Unsupported("magnet links"))
                    }
                } else {
                    Ok(request)
                }
            }
        }
    }

    async fn filter_request(
        &self,
        request: Request,
        acl: &Acl,
        current_rpc_request: &hyper::Request<Body>,
    ) -> Result<Request, FilterError> {
        let tag = request.tag;

        self.do_filter_request(request, acl, current_rpc_request)
            .await
            .map_err(|kind| FilterError { tag, kind })
    }

    fn do_filter_response(
        &self,
        request: &Request,
        response: RawResponse,
        acl: &Acl,
    ) -> Result<Response, FilterErrorKind> {
        if let Some(download_dir) = &acl.download_dir {
            match &request.call {
                MethodCall::TorrentGet { .. } => {
                    if let Some(torrent_get_raw) = response.arguments {
                        let mut torrents: Torrents = serde_json::from_value(torrent_get_raw)?;

                        torrents.torrents = torrents
                            .torrents
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

                        return Ok(Response {
                            tag: request.tag,
                            arguments: ResponseKind::Torrents(torrents).into(),
                            result: response.result,
                        });
                    }
                }

                MethodCall::SessionGet { .. } => {
                    if let Some(session_arguments_raw) = response.arguments {
                        let mut session: SessionArguments =
                            serde_json::from_value(session_arguments_raw)?;

                        session.download_dir = download_dir.to_owned();

                        return Ok(Response {
                            tag: request.tag,
                            arguments: ResponseKind::Session(session).into(),
                            result: response.result,
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(Response {
            tag: response.tag,
            arguments: response
                .arguments
                .map(|raw| ResponseKind::Other { extra: raw }),
            result: response.result,
        })
    }

    pub fn filter_response(
        &self,
        request: &Request,
        response: RawResponse,
        acl: &Acl,
    ) -> Result<Response, FilterError> {
        self.do_filter_response(request, response, acl)
            .map_err(|kind| {
                error!(request=?request, err=?kind, "error filtering response");

                FilterError {
                    tag: request.tag,
                    kind,
                }
            })
    }

    fn get_upstream_url(&self, req_url: &Uri) -> Uri {
        let mut parts = self.upstream.clone().into_parts();

        // TODO: Combine upstream path instead of replacing
        parts.path_and_query = req_url.path_and_query().cloned();

        // TODO: Handle possible errors
        Uri::from_parts(parts).expect("failed building upstream uri")
    }

    async fn forward_rpc_request_acl(
        &self,
        mut req: hyper::Request<Body>,
        acl: &Acl,
    ) -> Result<hyper::Response<Body>, hyper::Error> {
        // Parse the request body
        let req_body_bytes = hyper::body::to_bytes(req.body_mut()).await?;
        *req.body_mut() = Body::from(req_body_bytes.clone());

        let request = match serde_json::from_slice::<Request>(&req_body_bytes) {
            Ok(rpc_request) => {
                // Check that torrent add respects the download dir
                match self.filter_request(rpc_request, acl, &req).await {
                    Ok(request) => {
                        // Replace body
                        *req.body_mut() = Body::from(serde_json::to_string(&request).unwrap());
                        req.headers_mut().remove(CONTENT_LENGTH);

                        // Return the request object
                        request
                    }
                    Err(err) => {
                        return Ok(err.into());
                    }
                }
            }

            Err(err) => {
                warn!(%err, body = %String::from_utf8_lossy(&req_body_bytes), "could not parse request body");
                return Ok(FilterError {
                    tag: None,
                    kind: FilterErrorKind::ParseBody,
                }
                .into());
            }
        };

        // Fetch response
        let mut response = self.client.request(req).await?;
        debug!(?response);

        // Decode the response body
        let mut bytes = hyper::body::to_bytes(response.body_mut()).await?.to_vec();

        // HTTP 409 is used by transmission to exchange session keys
        if response.status() != 409 {
            // Perform replacements in RPC response
            if let Some::<RawResponse>(rpc_response) = serde_json::from_slice(&bytes)
                .map_err(|err| {
                    error!(?err);
                })
                .ok()
            {
                let response;
                bytes = serde_json::to_string(
                    match self.filter_response(&request, rpc_response, acl) {
                        Ok(resp) => {
                            response = resp;
                            &response
                        }
                        Err(err) => {
                            return Ok(err.into());
                        }
                    },
                )
                .expect("failed to serialize response")
                .into();
            }
        }

        // Replace response body and return response
        let (mut parts, _) = response.into_parts();
        parts.headers.remove(CONTENT_LENGTH);
        Ok(hyper::Response::from_parts(parts, Body::from(bytes)))
    }

    pub async fn handle_request(
        &self,
        mut req: hyper::Request<Body>,
        acl: Option<&Acl>,
    ) -> Result<hyper::Response<Body>, hyper::Error> {
        // Update target url
        *req.uri_mut() = self.get_upstream_url(req.uri());
        req.headers_mut().remove(HOST);

        if req.uri().path().ends_with("/rpc") {
            if let Some(acl) = acl {
                // We don't accept gzip to simplify things for rpc mapping
                req.headers_mut().remove(ACCEPT_ENCODING);

                return self.forward_rpc_request_acl(req, acl).await;
            }
        }

        Ok(self.client.request(req).await?)
    }
}
