use std::collections::HashMap;

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::warn;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
pub enum AuthUser {
    Anonymous,
    Basic {
        username: String,
        password: Option<SecretString>,
    },
    OAuth2 {
        username: String,
        provider: String,
    },
}

impl AuthUser {
    pub fn is_anonymous(&self) -> bool {
        matches!(self, AuthUser::Anonymous)
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
    #[serde(default = "default_true")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Provider {
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub visible: bool,

    pub client_id: oauth2::ClientId,
    pub client_secret: oauth2::ClientSecret,
    pub auth_url: oauth2::AuthUrl,
    pub token_url: oauth2::TokenUrl,
    pub userinfo_url: url::Url,
    pub email_path: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Providers {
    #[serde(default)]
    pub basic: BasicAuthProvider,
    #[serde(default)]
    pub oauth2: Vec<OAuth2Provider>,
}
