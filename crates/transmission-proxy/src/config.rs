use serde::{Deserialize, Serialize};

use crate::{acl::Acls, auth::Providers};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// List of ACLs
    pub acl: Acls,

    /// List of identity providers
    #[serde(default)]
    pub providers: Providers,
}
