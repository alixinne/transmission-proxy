use std::sync::Arc;

use axum::{
    async_trait,
    extract::{
        rejection::{TypedHeaderRejection, TypedHeaderRejectionReason},
        FromRequest, RequestParts, TypedHeader,
    },
    headers::{authorization::Basic, Authorization},
    response::{IntoResponse, Response},
    Extension,
};
use hyper::StatusCode;
use jwt::{SignWithKey, VerifyWithKey};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tower_cookies::Cookies;
use tracing::error;

use crate::{
    auth::AuthUser,
    server::{Ctx, JwtKey},
};

pub const COOKIE_NAME: &str = "_transmission_proxy";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserClaim {
    Basic { username: String },
    OAuth2 { username: String, provider: String },
}

impl UserClaim {
    pub fn from_auth_user(value: &AuthUser) -> Option<Self> {
        match value {
            AuthUser::Anonymous => None,
            AuthUser::Basic {
                username,
                password: _,
            } => Some(Self::Basic {
                username: username.clone(),
            }),
            AuthUser::OAuth2 { username, provider } => Some(Self::OAuth2 {
                username: username.clone(),
                provider: provider.clone(),
            }),
        }
    }

    pub fn jwt(&self, key: &JwtKey) -> String {
        self.sign_with_key(key).expect("failed to sign jwt")
    }

    pub fn verify(key: &JwtKey, jwt: &str) -> Result<Self, jwt::Error> {
        jwt.verify_with_key(key)
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
                username, provider, ..
            } => Self::OAuth2 { username, provider },
        }
    }
}

#[derive(Debug, Error)]
pub enum AuthenticationError {
    #[error(transparent)]
    InvalidAuthHeader(#[from] TypedHeaderRejection),
    #[error("invalid credentials for {0}")]
    InvalidCredentials(String),
    #[error("cookies error")]
    Cookies((StatusCode, &'static str)),
    #[error("invalid claim")]
    Jwt(#[from] jwt::Error),
}

impl IntoResponse for AuthenticationError {
    fn into_response(self) -> Response {
        match self {
            AuthenticationError::InvalidAuthHeader(err) => err.into_response(),
            AuthenticationError::InvalidCredentials(_) => {
                (StatusCode::UNAUTHORIZED, "invalid credentials").into_response()
            }
            AuthenticationError::Cookies(err) => err.into_response(),
            AuthenticationError::Jwt(_) => {
                (StatusCode::UNAUTHORIZED, "invalid claim").into_response()
            }
        }
    }
}

#[async_trait]
impl<B> FromRequest<B> for AuthUser
where
    B: Send,
{
    type Rejection = AuthenticationError;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Extension(ctx) = Extension::<Arc<Ctx>>::from_request(req)
            .await
            .expect("missing ctx");

        // Try to check auth cookie
        let cookies = Cookies::from_request(req)
            .await
            .map_err(AuthenticationError::Cookies)?;

        if let Some(cookie) = cookies.get(COOKIE_NAME) {
            return match UserClaim::verify(&ctx.jwt_key, cookie.value()) {
                Ok(claim) => Ok(claim.into()),
                Err(err) => Err(err.into()),
            };
        }

        if ctx.config.providers.basic.enabled {
            // Try to get basic auth information
            match TypedHeader::<Authorization<Basic>>::from_request(req).await {
                Ok(TypedHeader(Authorization(basic))) => {
                    let password: SecretString = basic.password().to_owned().into();

                    if ctx
                        .config
                        .providers
                        .basic
                        .auth(basic.username(), &password)
                        .await
                    {
                        Ok(Self::Basic {
                            username: basic.username().to_owned(),
                            password: Some(password),
                        })
                    } else {
                        Err(AuthenticationError::InvalidCredentials(
                            basic.username().to_owned(),
                        ))
                    }
                }

                Err(err) => match err.reason() {
                    TypedHeaderRejectionReason::Missing => Ok(Self::Anonymous),
                    _ => Err(err.into()),
                },
            }
        } else {
            Ok(Self::Anonymous)
        }
    }
}
