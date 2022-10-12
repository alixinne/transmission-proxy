use thiserror::Error;

use crate::types::*;

pub struct Client {
    rpc_url: url::Url,
    client: reqwest::Client,
    state: ClientState,
    tag: i32,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("the response type does not match the request")]
    ResponseTypeMismatch,
    #[error("the response tag does not match the request tag")]
    TagMismatch,
    #[error("could not acquire session id")]
    NoSessionId,
    #[error(transparent)]
    UnicodeError(#[from] reqwest::header::ToStrError),
    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
}

const SESSION_ID_HEADER: &str = "X-Transmission-Session-Id";

#[derive(Debug, Clone, PartialEq, Eq)]
enum ClientState {
    NoSession,
    HasSession(String),
}

impl ClientState {
    fn get_session_id(&self) -> Result<&str> {
        match self {
            Self::HasSession(id) => Ok(id.as_str()),
            _ => Err(Error::NoSessionId),
        }
    }
}

impl Default for ClientState {
    fn default() -> Self {
        Self::NoSession
    }
}

macro_rules! rpc_call {
    ($self:ident, $call:expr, $resp:path) => {
        Ok(match $self.rpc_call($call).await?.arguments {
            Some($resp(value)) => value,
            _ => {
                return Err(Error::ResponseTypeMismatch);
            }
        })
    };
}

impl Client {
    pub fn new(rpc_url: impl reqwest::IntoUrl) -> Result<Self> {
        Self::with_client(rpc_url, reqwest::Client::new())
    }

    pub fn with_client(rpc_url: impl reqwest::IntoUrl, client: reqwest::Client) -> Result<Self> {
        Ok(Self {
            rpc_url: rpc_url.into_url()?,
            client,
            state: Default::default(),
            tag: 57680,
        })
    }

    async fn rpc_call(&mut self, call: MethodCall) -> Result<Response> {
        // Check that we have a session id
        match self.state {
            ClientState::NoSession => {
                let response = self.client.post(self.rpc_url.clone()).send().await?;
                if let Some(session_id_value) = response.headers().get(SESSION_ID_HEADER) {
                    self.state = ClientState::HasSession(session_id_value.to_str()?.to_owned());
                }
            }
            ClientState::HasSession(_) => {}
        }

        // Get session id
        let session_id = self.state.get_session_id()?;

        // Build request
        let request = Request {
            call,
            tag: Some(self.tag),
        };

        // Increment tag for next requests
        self.tag += 1;

        let response: Response = self
            .client
            .post(self.rpc_url.clone())
            .header(SESSION_ID_HEADER, session_id)
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        if response.tag != request.tag {
            return Err(Error::TagMismatch);
        }

        Ok(response)
    }

    pub async fn session_get(&mut self, arguments: SessionGet) -> Result<SessionArguments> {
        rpc_call!(
            self,
            MethodCall::SessionGet { arguments },
            ResponseKind::Session
        )
    }

    pub async fn torrent_get(&mut self, arguments: TorrentGet) -> Result<Torrents> {
        rpc_call!(
            self,
            MethodCall::TorrentGet { arguments },
            ResponseKind::Torrents
        )
    }
}
