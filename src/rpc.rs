use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RpcConfig {
    pub download_dir: String,

    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcTorrent {
    pub download_dir: String,

    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcResponseKind {
    Config(RpcConfig),
    Torrents {
        torrents: Vec<RpcTorrent>,
        #[serde(flatten)]
        extra: HashMap<String, serde_json::Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum RpcResponseStatus {
    Success,
    Failure(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<RpcResponseKind>,
    pub result: RpcResponseStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcArguments {
    #[serde(flatten)]
    pub args: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RpcTorrentAdd {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookies: Option<String>,
    pub download_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    pub metainfo: String,
    pub paused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_limit: Option<i32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "bandwidthPriority"
    )]
    pub bandwidth_priority: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_wanted: Vec<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_unwanted: Vec<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priority_high: Vec<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priority_low: Vec<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priority_normal: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, strum::EnumDiscriminants)]
#[serde(rename_all = "kebab-case", tag = "method")]
#[strum_discriminants(
    derive(Serialize, Deserialize),
    name(RpcMethodName),
    serde(rename_all = "kebab-case")
)]
pub enum RpcMethodCall {
    TorrentStart { arguments: RpcArguments },
    TorrentStartNow { arguments: RpcArguments },
    TorrentStop { arguments: RpcArguments },
    TorrentVerify { arguments: RpcArguments },
    TorrentReannounce { arguments: RpcArguments },
    TorrentSet { arguments: RpcArguments },
    TorrentGet { arguments: RpcArguments },
    TorrentAdd { arguments: RpcTorrentAdd },
    TorrentRemove { arguments: RpcArguments },
    TorrentSetLocation { arguments: RpcArguments },
    TorrentRenamePath { arguments: RpcArguments },
    SessionSet { arguments: RpcArguments },
    SessionGet { arguments: Option<RpcArguments> },
    SessionStats { arguments: Option<RpcArguments> },
    BlocklistUpdate { arguments: RpcArguments },
    PortTest { arguments: RpcArguments },
    SessionClose { arguments: RpcArguments },
    QueueMoveTop { arguments: RpcArguments },
    QueueMoveUp { arguments: RpcArguments },
    QueueMoveDown { arguments: RpcArguments },
    QueueMoveBottom { arguments: RpcArguments },
    FreeSpace { arguments: RpcArguments },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    #[serde(flatten)]
    pub call: RpcMethodCall,
    pub tag: Option<i32>,
}
