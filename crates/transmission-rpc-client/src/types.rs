use std::{borrow::Cow, collections::HashMap};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IntBool {
    Int(i32),
    Bool(bool),
}

impl IntBool {
    pub fn as_bool(&self) -> bool {
        (*self).into()
    }
}

impl Default for IntBool {
    fn default() -> Self {
        Self::Bool(false)
    }
}

impl From<bool> for IntBool {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i32> for IntBool {
    fn from(value: i32) -> Self {
        Self::Int(value)
    }
}

impl From<IntBool> for bool {
    fn from(value: IntBool) -> Self {
        match value {
            IntBool::Int(i) => i != 0,
            IntBool::Bool(b) => b,
        }
    }
}

impl From<IntBool> for i32 {
    fn from(value: IntBool) -> Self {
        match value {
            IntBool::Int(i) => i,
            IntBool::Bool(b) => i32::from(b),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TorrentId {
    Id(i32),
    Sha1(String),
}

impl From<i32> for TorrentId {
    fn from(value: i32) -> Self {
        Self::Id(value)
    }
}

impl From<String> for TorrentId {
    fn from(value: String) -> Self {
        Self::Sha1(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TorrentIdSet {
    RecentlyActive,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TorrentIds {
    Id(i32),
    Ids(Vec<TorrentId>),
    Set(TorrentIdSet),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Torrent {
    #[serde(default)]
    pub id: Option<TorrentId>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default)]
    pub download_dir: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trackers: Option<Vec<Tracker>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracker_stats: Option<Vec<TrackerStats>>,

    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tracker {
    pub id: i32,
    pub tier: i32,
    pub announce: String,
    pub scrape: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackerStats {
    pub id: i32,
    pub tier: i32,
    pub announce: String,
    pub scrape: String,

    pub has_announced: bool,
    pub has_scraped: bool,
    pub host: String,
    pub is_backup: bool,
    pub last_announce_peer_count: i32,
    pub last_announce_result: String,
    pub last_announce_succeeded: bool,
    pub last_announce_timed_out: bool,
    pub last_scrape_result: String,
    pub last_scrape_succeeded: bool,

    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum ResponseKind {
    Torrents(Torrents),
    Session(SessionArguments),
    SessionStats(SessionStats),
    Other {
        #[serde(flatten)]
        extra: serde_json::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ResponseStatus {
    Success,
    Failure(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
    pub result: ResponseStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<ResponseKind>,
    pub result: ResponseStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Torrents {
    pub torrents: Vec<Torrent>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SessionArguments {
    /// max global download speed (KBps)
    pub alt_speed_down: i32,
    /// true means use the alt speeds
    pub alt_speed_enabled: IntBool,
    /// when to turn on alt speeds (units: minutes after midnight)
    pub alt_speed_time_begin: i32,
    /// what day(s) to turn on alt speeds (look at tr_sched_day)
    pub alt_speed_time_day: i32,
    /// true means the scheduled on/off times are used
    pub alt_speed_time_enabled: IntBool,
    /// when to turn off alt speeds (units: same)
    pub alt_speed_time_end: i32,
    /// max global upload speed (KBps)
    pub alt_speed_up: i32,
    /// true means enabled
    pub blocklist_enabled: IntBool,
    /// i32 of rules in the blocklist
    pub blocklist_size: i32,
    /// location of the blocklist to use for blocklist-update
    pub blocklist_url: String,
    /// maximum size of the disk cache (MB)
    pub cache_size_mb: i32,
    /// location of transmission's configuration directory
    pub config_dir: String,
    /// true means allow dht in public torrents
    pub dht_enabled: IntBool,
    /// default path to download torrents
    pub download_dir: String,
    /// if true, limit how many torrents can be downloaded at once
    pub download_queue_enabled: IntBool,
    /// max i32 of torrents to download at once (see download-queue-enabled)
    pub download_queue_size: i32,
    /// required, preferred, tolerated
    pub encryption: String,
    /// true if the seeding inactivity limit is honored by default
    pub idle_seeding_limit_enabled: IntBool,
    /// torrents we're seeding will be stopped if they're idle for this long
    pub idle_seeding_limit: i32,
    /// true means keep torrents in incomplete-dir until done
    pub incomplete_dir_enabled: IntBool,
    /// path for incomplete torrents, when enabled
    pub incomplete_dir: String,
    /// true means allow Local Peer Discovery in public torrents
    pub lpd_enabled: IntBool,
    /// maximum global i32 of peers
    pub peer_limit_global: i32,
    /// maximum global i32 of peers
    pub peer_limit_per_torrent: i32,
    /// true means pick a random peer port on launch
    pub peer_port_random_on_start: IntBool,
    /// port i32
    pub peer_port: i32,
    /// true means allow pex in public torrents
    pub pex_enabled: IntBool,
    /// true means ask upstream router to forward the configured peer port to transmission using UPnP or NAT-PMP
    pub port_forwarding_enabled: IntBool,
    /// whether or not to consider idle torrents as stalled
    pub queue_stalled_enabled: IntBool,
    /// torrents that are idle for N minuets aren't counted toward seed-queue-size or download-queue-size
    pub queue_stalled_minutes: i32,
    /// true means append .part to incomplete files
    pub rename_partial_files: IntBool,
    /// the minimum RPC API version supported
    pub rpc_version_minimum: i32,
    /// the current RPC API version in a semver-compatible string
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpc_version_semver: Option<i32>,
    /// the current RPC API version
    pub rpc_version: i32,
    /// whether or not to call the added script
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_torrent_added_enabled: Option<bool>,
    /// filename of the script to run
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_torrent_added_filename: Option<String>,
    /// whether or not to call the done script
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_torrent_done_enabled: Option<bool>,
    /// filename of the script to run
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_torrent_done_filename: Option<String>,
    /// whether or not to call the seeding-done script
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_torrent_done_seeding_enabled: Option<bool>,
    /// filename of the script to run
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_torrent_done_seeding_filename: Option<String>,
    /// if true, limit how many torrents can be uploaded at once
    pub seed_queue_enabled: IntBool,
    /// max i32 of torrents to uploaded at once (see seed-queue-enabled)
    pub seed_queue_size: i32,
    /// default seed ratio for torrents to use
    #[serde(rename = "seedRatioLimit")]
    pub seed_ratio_limit: f32,
    /// true if seedRatioLimit is honored by default
    #[serde(rename = "seedRatioLimited")]
    pub seed_ratio_limited: IntBool,
    /// true means enabled
    pub speed_limit_down_enabled: IntBool,
    /// max global download speed (KBps)
    pub speed_limit_down: i32,
    /// true means enabled
    pub speed_limit_up_enabled: IntBool,
    /// max global upload speed (KBps)
    pub speed_limit_up: i32,
    /// true means added torrents will be started right away
    pub start_added_torrents: IntBool,
    /// true means the .torrent file of added torrents will be deleted
    pub trash_original_torrent_files: IntBool,
    /// see below
    pub units: SessionUnits,
    /// true means allow utp
    pub utp_enabled: IntBool,
    /// long version string $version ($revision)
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub active_torrent_count: i32,
    pub download_speed: i32,
    pub paused_torrent_count: i32,
    pub torrent_count: i32,
    pub upload_speed: i32,
    #[serde(rename = "cumulative-stats")]
    pub cumulative_stats: Stats,
    #[serde(rename = "current-stats")]
    pub current_stats: Stats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub uploaded_bytes: i32,
    pub downloaded_bytes: i32,
    pub files_added: i32,
    pub session_count: i32,
    pub seconds_active: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SessionSet {
    /// max global download speed (KBps)
    pub alt_speed_down: i32,
    /// true means use the alt speeds
    pub alt_speed_enabled: IntBool,
    /// when to turn on alt speeds (units: minutes after midnight)
    pub alt_speed_time_begin: i32,
    /// what day(s) to turn on alt speeds (look at tr_sched_day)
    pub alt_speed_time_day: i32,
    /// true means the scheduled on/off times are used
    pub alt_speed_time_enabled: IntBool,
    /// when to turn off alt speeds (units: same)
    pub alt_speed_time_end: i32,
    /// max global upload speed (KBps)
    pub alt_speed_up: i32,
    /// true means enabled
    pub blocklist_enabled: IntBool,
    /// location of the blocklist to use for blocklist-update
    pub blocklist_url: String,
    /// maximum size of the disk cache (MB)
    pub cache_size_mb: i32,
    /// true means allow dht in public torrents
    pub dht_enabled: IntBool,
    /// default path to download torrents
    pub download_dir: String,
    /// if true, limit how many torrents can be downloaded at once
    pub download_queue_enabled: IntBool,
    /// max i32 of torrents to download at once (see download-queue-enabled)
    pub download_queue_size: i32,
    /// required, preferred, tolerated
    pub encryption: String,
    /// true if the seeding inactivity limit is honored by default
    pub idle_seeding_limit_enabled: IntBool,
    /// torrents we're seeding will be stopped if they're idle for this long
    pub idle_seeding_limit: i32,
    /// true means keep torrents in incomplete-dir until done
    pub incomplete_dir_enabled: IntBool,
    /// path for incomplete torrents, when enabled
    pub incomplete_dir: String,
    /// true means allow Local Peer Discovery in public torrents
    pub lpd_enabled: IntBool,
    /// maximum global i32 of peers
    pub peer_limit_global: i32,
    /// maximum global i32 of peers
    pub peer_limit_per_torrent: i32,
    /// true means pick a random peer port on launch
    pub peer_port_random_on_start: IntBool,
    /// port i32
    pub peer_port: i32,
    /// true means allow pex in public torrents
    pub pex_enabled: IntBool,
    /// true means ask upstream router to forward the configured peer port to transmission using UPnP or NAT-PMP
    pub port_forwarding_enabled: IntBool,
    /// whether or not to consider idle torrents as stalled
    pub queue_stalled_enabled: IntBool,
    /// torrents that are idle for N minuets aren't counted toward seed-queue-size or download-queue-size
    pub queue_stalled_minutes: i32,
    /// true means append .part to incomplete files
    pub rename_partial_files: IntBool,
    /// whether or not to call the added script
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_torrent_added_enabled: Option<bool>,
    /// filename of the script to run
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_torrent_added_filename: Option<String>,
    /// whether or not to call the done script
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_torrent_done_enabled: Option<bool>,
    /// filename of the script to run
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_torrent_done_filename: Option<String>,
    /// whether or not to call the seeding-done script
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_torrent_done_seeding_enabled: Option<bool>,
    /// filename of the script to run
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_torrent_done_seeding_filename: Option<String>,
    /// if true, limit how many torrents can be uploaded at once
    pub seed_queue_enabled: IntBool,
    /// max i32 of torrents to uploaded at once (see seed-queue-enabled)
    pub seed_queue_size: i32,
    /// default seed ratio for torrents to use
    #[serde(rename = "seedRatioLimit")]
    pub seed_ratio_limit: f32,
    /// true if seedRatioLimit is honored by default
    #[serde(rename = "seedRatioLimited")]
    pub seed_ratio_limited: IntBool,
    /// true means enabled
    pub speed_limit_down_enabled: IntBool,
    /// max global download speed (KBps)
    pub speed_limit_down: i32,
    /// true means enabled
    pub speed_limit_up_enabled: IntBool,
    /// max global upload speed (KBps)
    pub speed_limit_up: i32,
    /// true means added torrents will be started right away
    pub start_added_torrents: IntBool,
    /// true means the .torrent file of added torrents will be deleted
    pub trash_original_torrent_files: IntBool,
    /// see below
    pub units: SessionUnits,
    /// true means allow utp
    pub utp_enabled: IntBool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SessionUnits {
    pub speed_units: Vec<String>,
    pub speed_bytes: i32,
    pub size_units: Vec<String>,
    pub size_bytes: i32,
    pub memory_units: Vec<String>,
    pub memory_bytes: i32,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SessionGet {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Cow<'static, str>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TorrentAdd {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookies: Option<String>,
    pub download_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    pub metainfo: String,
    pub paused: IntBool,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TorrentAction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ids: Option<TorrentIds>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TorrentSet {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "bandwidthPriority"
    )]
    pub bandwidth_priority: Option<i32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "downloadLimit"
    )]
    pub download_limit: Option<i32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "downloadLimited"
    )]
    pub download_limited: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_wanted: Vec<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_unwanted: Vec<i32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "honorsSessionLimits"
    )]
    pub honors_session_limits: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ids: Option<TorrentIds>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_limit: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priority_high: Vec<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priority_low: Vec<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priority_normal: Vec<i32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "queuePosition"
    )]
    pub queue_position: Option<i32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "seedIdleLimit"
    )]
    pub seed_idle_limit: Option<i32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "seedIdleMode"
    )]
    pub seed_idle_mode: Option<i32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "seedRatioLimit"
    )]
    pub seed_ratio_limit: Option<f32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "seedRatioMode"
    )]
    pub seed_ratio_mode: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "trackerAdd")]
    pub tracker_add: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "trackerRemove"
    )]
    pub tracker_remove: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "trackerReplace"
    )]
    pub tracker_replace: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "uploadLimit"
    )]
    pub upload_limit: Option<i32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "uploadLimited"
    )]
    pub upload_limited: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TorrentGetFormat {
    Objects,
    Table,
}

impl TorrentGetFormat {
    pub fn is_objects(&self) -> bool {
        matches!(self, Self::Objects)
    }
}

impl Default for TorrentGetFormat {
    fn default() -> Self {
        Self::Objects
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TorrentGet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ids: Option<TorrentIds>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Cow<'static, str>>,
    #[serde(default, skip_serializing_if = "TorrentGetFormat::is_objects")]
    pub format: TorrentGetFormat,
}

impl Default for TorrentGet {
    fn default() -> Self {
        Self {
            ids: None,
            fields: vec![
                Cow::Borrowed("id"),
                Cow::Borrowed("addedDate"),
                Cow::Borrowed("name"),
                Cow::Borrowed("totalSize"),
                Cow::Borrowed("error"),
                Cow::Borrowed("errorString"),
                Cow::Borrowed("eta"),
                Cow::Borrowed("isFinished"),
                Cow::Borrowed("isStalled"),
                Cow::Borrowed("leftUntilDone"),
                Cow::Borrowed("metadataPercentComplete"),
                Cow::Borrowed("peersConnected"),
                Cow::Borrowed("peersGettingFromUs"),
                Cow::Borrowed("peersSendingToUs"),
                Cow::Borrowed("percentDone"),
                Cow::Borrowed("queuePosition"),
                Cow::Borrowed("rateDownload"),
                Cow::Borrowed("rateUpload"),
                Cow::Borrowed("recheckProgress"),
                Cow::Borrowed("seedRatioMode"),
                Cow::Borrowed("seedRatioLimit"),
                Cow::Borrowed("sizeWhenDone"),
                Cow::Borrowed("status"),
                Cow::Borrowed("trackers"),
                Cow::Borrowed("downloadDir"),
                Cow::Borrowed("uploadedEver"),
                Cow::Borrowed("uploadRatio"),
                Cow::Borrowed("webseedsSendingToUs"),
            ],
            format: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TorrentRemove {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ids: Option<TorrentIds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete_local_data: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TorrentSetLocation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ids: Option<TorrentIds>,
    pub location: String,
    #[serde(default, rename = "move")]
    pub move_data: IntBool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TorrentRenamePath {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ids: Option<TorrentIds>,
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct QueueMovement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ids: Option<TorrentIds>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreeSpace {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, strum::EnumDiscriminants)]
#[serde(rename_all = "kebab-case", tag = "method")]
#[strum_discriminants(
    derive(Serialize, Deserialize),
    name(MethodName),
    serde(rename_all = "kebab-case")
)]
pub enum MethodCall {
    TorrentStart {
        arguments: TorrentAction,
    },
    TorrentStartNow {
        arguments: TorrentAction,
    },
    TorrentStop {
        arguments: TorrentAction,
    },
    TorrentVerify {
        arguments: TorrentAction,
    },
    TorrentReannounce {
        arguments: TorrentAction,
    },
    TorrentSet {
        arguments: TorrentSet,
    },
    TorrentGet {
        arguments: TorrentGet,
    },
    TorrentAdd {
        arguments: TorrentAdd,
    },
    TorrentRemove {
        arguments: TorrentRemove,
    },
    TorrentSetLocation {
        arguments: TorrentSetLocation,
    },
    TorrentRenamePath {
        arguments: TorrentRenamePath,
    },
    SessionSet {
        arguments: SessionSet,
    },
    SessionGet {
        #[serde(default)]
        arguments: SessionGet,
    },
    SessionStats,
    BlocklistUpdate,
    PortTest,
    SessionClose,
    QueueMoveTop {
        arguments: QueueMovement,
    },
    QueueMoveUp {
        arguments: QueueMovement,
    },
    QueueMoveDown {
        arguments: QueueMovement,
    },
    QueueMoveBottom {
        arguments: QueueMovement,
    },
    FreeSpace {
        arguments: FreeSpace,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    #[serde(flatten)]
    pub call: MethodCall,
    pub tag: Option<i32>,
}
