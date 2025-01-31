use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthUser, Providers},
    rpc,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Acls {
    rules: Vec<Acl>,
}

impl Acls {
    fn get_anon(&self) -> Option<&Acl> {
        self.rules.iter().find(|acl| acl.identities.is_empty())
    }

    pub async fn get(&self, user: &AuthUser, providers: &Providers) -> Option<&Acl> {
        match user {
            AuthUser::Anonymous => None,
            AuthUser::Basic { username, password } => {
                let basic_user = self.rules.iter().find(|acl| {
                    // Find a matching identity
                    acl.identities.iter().any(|identity| match identity {
                        AclIdentity::Basic { name } => name == username.as_str(),
                        _ => false,
                    })
                });

                let basic_user = basic_user?;

                if let Some(password) = password {
                    providers
                        .basic
                        .auth(username, password)
                        .await
                        .then_some(basic_user)
                } else {
                    // Auth through JWT
                    Some(basic_user)
                }
            }
            AuthUser::OAuth2 { username, provider } => {
                let oauth_user = self.rules.iter().find(|acl| {
                    // Find a matching identity
                    acl.identities.iter().any(|identity| match identity {
                        AclIdentity::OAuth2 { name, oauth2 } => {
                            name == username.as_str() && oauth2 == provider
                        }
                        _ => false,
                    })
                });

                oauth_user
            }
        }
        .or_else(|| self.get_anon())
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase", tag = "provider", deny_unknown_fields)]
pub enum AclIdentity {
    Basic { name: String },
    OAuth2 { name: String, oauth2: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Acl {
    /// List of identities concerned by this ACL
    #[serde(default)]
    pub identities: HashSet<AclIdentity>,

    /// Forced download dir for this ACL
    pub download_dir: Option<String>,

    /// List of allowed RPC methods. Unrestricted if empty (use deny to block access).
    #[serde(default)]
    pub allowed_methods: Vec<rpc::MethodName>,

    /// Deny all access to matched members
    #[serde(default)]
    pub deny: bool,

    /// Tracker rules
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracker_rules: Vec<TrackerRule>,
}

impl Acl {
    /// Returns true if this ACL does not filter anything (all requests, download dirs and methods
    /// are allowed). This is used to skip request deserialization if needed.
    pub fn is_nop(&self) -> bool {
        self.download_dir.is_none()
            && self.allowed_methods.is_empty()
            && !self.deny
            && self.tracker_rules.is_empty()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TrackerRule {
    Replace {
        #[serde(with = "serde_regex")]
        from: regex::Regex,
        to: String,
    },
}

impl TrackerRule {
    pub fn matches(&self, _announce: &str) -> bool {
        match self {
            TrackerRule::Replace { .. } => true,
        }
    }

    pub fn apply(&self, announce: &str) -> Option<String> {
        match self {
            TrackerRule::Replace { from, to } => Some(from.replace(announce, to).to_string()),
        }
    }
}
