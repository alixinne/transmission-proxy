use std::collections::HashMap;

use cookie::Cookie;
use hyper::{
    header::{AUTHORIZATION, COOKIE},
    Request,
};
use secrecy::SecretString;
use tracing::warn;

#[derive(Debug, thiserror::Error)]
pub enum ParseRequestError {}

pub trait RequestExt {
    fn parse(&self) -> Result<ParsedRequest, ParseRequestError>;
}

impl<B> RequestExt for Request<B> {
    fn parse(&self) -> Result<ParsedRequest, ParseRequestError> {
        self.try_into()
    }
}

pub struct BasicUser {
    pub username: String,
    pub password: SecretString,
}

pub struct ParsedRequest {
    pub basic_auth: Option<BasicUser>,
    pub query_parameters: HashMap<String, String>,
    pub cookies: HashMap<String, Cookie<'static>>,
}

impl<B> TryFrom<&Request<B>> for ParsedRequest {
    type Error = ParseRequestError;

    fn try_from(req: &Request<B>) -> Result<Self, Self::Error> {
        // Get basic auth information
        let basic_auth = if let Some(authorization) = req.headers().get(AUTHORIZATION) {
            if let Ok(value_string) = authorization.to_str() {
                let parts: Vec<_> = value_string.splitn(2, ' ').collect();

                if parts.len() == 2 {
                    if parts[0] == "Basic" {
                        match base64::decode(parts[1]) {
                            Ok(bytes) => match String::from_utf8(bytes.to_vec()) {
                                Ok(basic_auth_string) => {
                                    let parts: Vec<_> = basic_auth_string.splitn(2, ':').collect();

                                    if parts.len() == 2 {
                                        Some(BasicUser {
                                            username: parts[0].to_string(),
                                            password: parts[1].to_string().into(),
                                        })
                                    } else {
                                        warn!("invalid basic authorization string");
                                        None
                                    }
                                }
                                Err(err) => {
                                    warn!(%err, "invalid utf8 in basic authorization string");
                                    None
                                }
                            },
                            Err(err) => {
                                warn!(%err, "invalid basic authorization base64");
                                None
                            }
                        }
                    } else {
                        warn!(ty = %parts[0], "unsupported authorization type");
                        None
                    }
                } else {
                    warn!(header = %value_string, "invalid authorization header");
                    None
                }
            } else {
                warn!("invalid utf8 in authorization header");
                None
            }
        } else {
            None
        };

        // Parse query parameters
        let mut query_parameters = HashMap::new();

        if let Some(query) = req.uri().query() {
            for (name, value) in url::form_urlencoded::parse(query.as_bytes()) {
                match query_parameters.entry(name.to_string()) {
                    std::collections::hash_map::Entry::Occupied(_) => {
                        warn!(%name, "ignoring duplicated query parameter");
                    }

                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(value.to_string());
                    }
                }
            }
        }

        // Parse cookies
        let mut cookies = HashMap::new();

        if let Some(value) = req.headers().get(COOKIE) {
            match value.to_str() {
                Ok(value) => {
                    for cookie in value.split(';') {
                        match Cookie::parse(cookie.to_string()) {
                            Ok(parsed) => match cookies.entry(parsed.name().to_string()) {
                                std::collections::hash_map::Entry::Occupied(_) => {
                                    warn!(name = %parsed.name(), "ignoring duplicated cookie");
                                }

                                std::collections::hash_map::Entry::Vacant(entry) => {
                                    entry.insert(parsed);
                                }
                            },
                            Err(err) => {
                                warn!(%err, "invalid cookie");
                            }
                        }
                    }
                }
                Err(err) => {
                    warn!(%err, "invalid utf8 in cookie header");
                }
            }
        }

        Ok(Self {
            basic_auth,
            query_parameters,
            cookies,
        })
    }
}
