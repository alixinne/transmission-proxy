use std::collections::HashMap;

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::warn;

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
