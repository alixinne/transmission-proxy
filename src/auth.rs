use std::collections::HashMap;

use jwt::{SignWithKey, VerifyWithKey};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::warn;

use crate::{ext::ParsedRequest, server::JwtKey};

pub const COOKIE_NAME: &str = "_transmission_proxy";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserClaim {
    Basic { username: String },
    OAuth2 { username: String, provider: String },
}

impl UserClaim {
    pub fn jwt(&self, key: &JwtKey) -> String {
        self.sign_with_key(key).expect("failed to sign jwt")
    }

    pub fn verify(key: &JwtKey, jwt: &str) -> Result<Self, jwt::Error> {
        jwt.verify_with_key(key)
    }
}

#[derive(Debug, Clone)]
pub enum AuthUser {
    Anonymous,
    Basic {
        username: String,
        password: Option<SecretString>,
    },
}

impl AuthUser {
    pub fn is_anonymous(&self) -> bool {
        matches!(self, AuthUser::Anonymous)
    }

    pub fn claim(&self) -> Option<UserClaim> {
        match self {
            Self::Anonymous => None,
            Self::Basic {
                username,
                password: _,
            } => Some(UserClaim::Basic {
                username: username.clone(),
            }),
        }
    }

    pub fn auth(jwt_key: &JwtKey, parsed: &ParsedRequest) -> Self {
        // Check JWT in cookie
        if let Some(jwt) = parsed.cookies.get(COOKIE_NAME) {
            if let Ok(claim) = UserClaim::verify(jwt_key, jwt.value()) {
                return claim.into();
            }
        }

        // Check basic auth credentials, if any
        if let Some(user) = &parsed.basic_auth {
            return Self::Basic {
                username: user.username.clone(),
                password: Some(user.password.clone()),
            };
        }

        Self::Anonymous
    }
}

impl From<UserClaim> for AuthUser {
    fn from(claim: UserClaim) -> Self {
        match claim {
            UserClaim::Basic { username } => Self::Basic {
                username,
                password: None,
            },
            UserClaim::OAuth2 {
                username: _,
                provider: _,
            } => todo!(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicAuthUser {
    pub username: String,
    pub password: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicAuthProvider {
    pub enabled: bool,
    pub visible: bool,
    pub users: Vec<BasicAuthUser>,

    #[serde(skip)]
    verify_cache: Mutex<HashMap<String, SecretString>>,
}

impl BasicAuthProvider {
    pub async fn auth(&self, user: &str, password: &SecretString) -> bool {
        if let Some(basic_auth_user) = self.users.iter().find(|entry| entry.username == user) {
            let mut verify_cache = self.verify_cache.lock().await;

            // Check the cache first to skip bcrypt verification
            if let Some(already_verified) = verify_cache.get(&basic_auth_user.username) {
                return already_verified.expose_secret().as_str()
                    == password.expose_secret().as_str();
            }

            // If not found, verify with bcrypt
            match bcrypt::verify(
                password.expose_secret().as_bytes(),
                &basic_auth_user.password,
            ) {
                Ok(result) => {
                    if result {
                        verify_cache.insert(basic_auth_user.username.to_string(), password.clone());
                    }

                    return result;
                }
                Err(err) => {
                    warn!(%err, %user, "error verifying password");
                }
            }
        }

        false
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Providers {
    #[serde(default)]
    pub basic: BasicAuthProvider,
}
