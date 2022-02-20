use hyper::Uri;
use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not resolve {0} as a into an IP address and port")]
    BindResolve(Uri),
}
