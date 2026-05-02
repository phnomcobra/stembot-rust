use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::enums::ControlFormType;
use crate::models::routing::{Peer, Route};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn gen_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn unix_now_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn default_create_ticket() -> ControlFormType {
    ControlFormType::CreateTicket
}

// ── Command argument (str | List[str]) ───────────────────────────────────────

/// Maps to Python's `str | List[str]` command field in `SyncProcess`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommandArg {
    Single(String),
    Multi(Vec<String>),
}

impl Default for CommandArg {
    fn default() -> Self {
        Self::Single(String::new())
    }
}

// ── Individual control form structs ──────────────────────────────────────────
// These structs do NOT carry a `type` field; the type is encoded by the
// `ControlFormVariant` tagged enum when serialised as part of a ticket.

/// Request to load a file from a remote agent.
/// Maps to Python's `LoadFile(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoadFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b64zlib: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub md5sum: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// Request to write a file to a remote agent.
/// Maps to Python's `WriteFile(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WriteFile {
    pub b64zlib: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub md5sum: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// Request to synchronously execute a process on a remote agent.
/// Maps to Python's `SyncProcess(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProcess {
    pub command: CommandArg,
    #[serde(default = "default_sync_timeout")]
    pub timeout: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

fn default_sync_timeout() -> i64 { 15 }

/// Request to create a peer connection to another agent.
/// Maps to Python's `CreatePeer(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreatePeer {
    pub agtuuid: String,
    #[serde(default)]
    pub polling: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// Request to discover and create a peer connection via URL.
/// Maps to Python's `DiscoverPeer(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoverPeer {
    pub url: String,
    #[serde(default)]
    pub polling: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agtuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// Request to delete one or more peers.
/// Maps to Python's `DeletePeers(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeletePeers {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agtuuids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// Request to retrieve all current peer connections.
/// Maps to Python's `GetPeers(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetPeers {
    #[serde(default)]
    pub peers: Vec<Peer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// Request to retrieve the routing table.
/// Maps to Python's `GetRoutes(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetRoutes {
    #[serde(default)]
    pub routes: Vec<Route>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// Request to retrieve the agent configuration.
/// Maps to Python's `GetConfig(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

// ── Tagged union of all control form variants ─────────────────────────────────

/// Internally-tagged union of all control forms.
///
/// Serialises as `{ "type": "<TYPE>", ...fields }`.
/// Used as the `form` field in `ControlFormTicket` and `NetworkTicket`.
/// Maps to Python's `Union[CreatePeer, DiscoverPeer, ...]` field annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ControlFormVariant {
    #[serde(rename = "CREATE_PEER")]   CreatePeer(CreatePeer),
    #[serde(rename = "DISCOVER_PEER")] DiscoverPeer(DiscoverPeer),
    #[serde(rename = "DELETE_PEERS")]  DeletePeers(DeletePeers),
    #[serde(rename = "GET_PEERS")]     GetPeers(GetPeers),
    #[serde(rename = "GET_ROUTES")]    GetRoutes(GetRoutes),
    #[serde(rename = "SYNC_PROCESS")]  SyncProcess(SyncProcess),
    #[serde(rename = "WRITE_FILE")]    WriteFile(WriteFile),
    #[serde(rename = "LOAD_FILE")]     LoadFile(LoadFile),
    #[serde(rename = "GET_CONFIG")]    GetConfig(GetConfig),
}

// ── Hop ───────────────────────────────────────────────────────────────────────

/// A single hop in a ticket trace.
/// Maps to Python's `Hop(BaseModel)` in `models/control.py`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Hop {
    pub agtuuid:  String,
    pub hop_time: f64,
    pub type_str: String,
}

// ── ControlFormTicket ─────────────────────────────────────────────────────────

/// A ticket for asynchronous control form delivery.
/// Maps to Python's `ControlFormTicket(ControlForm)`.
///
/// Unlike the individual form structs, this struct carries its own `type`
/// field (serialised as `"type"`) since it is not embedded in a tagged enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlFormTicket {
    #[serde(rename = "type", default = "default_create_ticket")]
    pub form_type: ControlFormType,
    #[serde(default = "gen_uuid")]
    pub tckuuid: String,
    #[serde(default)]
    pub src: String,
    #[serde(default)]
    pub dst: String,
    #[serde(default = "unix_now_f64")]
    pub create_time: f64,
    #[serde(default)]
    pub tracing: bool,
    #[serde(default)]
    pub hops: Vec<Hop>,
    pub form: ControlFormVariant,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}
