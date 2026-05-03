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
// `ControlFormVariant` tagged enum when serialised.
// All Option fields serialize as null (no skip_serializing_if) to match
// the Python protocol wire format.

/// Request to load a file from a remote agent.
/// Maps to Python's `LoadFile(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoadFile {
    pub path:    String,
    pub b64zlib: Option<String>,
    pub size:    Option<i64>,
    pub md5sum:  Option<String>,
    pub error:   Option<String>,
    pub objuuid: Option<String>,
    pub coluuid: Option<String>,
}

/// Request to write a file to a remote agent.
/// Maps to Python's `WriteFile(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WriteFile {
    pub b64zlib: String,
    pub path:    String,
    pub size:    Option<i64>,
    pub md5sum:  Option<String>,
    pub error:   Option<String>,
    pub objuuid: Option<String>,
    pub coluuid: Option<String>,
}

/// Request to synchronously execute a process on a remote agent.
/// Maps to Python's `SyncProcess(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProcess {
    pub command:      CommandArg,
    #[serde(default = "default_sync_timeout")]
    pub timeout:      i64,
    pub stdout:       Option<String>,
    pub stderr:       Option<String>,
    pub status:       Option<i64>,
    pub start_time:   Option<f64>,
    pub elapsed_time: Option<f64>,
    pub error:        Option<String>,
    pub objuuid:      Option<String>,
    pub coluuid:      Option<String>,
}

fn default_sync_timeout() -> i64 { 15 }

/// Request to create a peer connection to another agent.
/// Maps to Python's `CreatePeer(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreatePeer {
    pub agtuuid: String,
    #[serde(default)]
    pub polling: bool,
    pub url:     Option<String>,
    pub ttl:     Option<f64>,
    pub error:   Option<String>,
    pub objuuid: Option<String>,
    pub coluuid: Option<String>,
}

/// Request to discover and create a peer connection via URL.
/// Maps to Python's `DiscoverPeer(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoverPeer {
    pub url:     String,
    #[serde(default)]
    pub polling: bool,
    pub agtuuid: Option<String>,
    pub ttl:     Option<f64>,
    pub error:   Option<String>,
    pub objuuid: Option<String>,
    pub coluuid: Option<String>,
}

/// Request to delete one or more peers.
/// Maps to Python's `DeletePeers(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeletePeers {
    pub agtuuids: Option<Vec<String>>,
    pub error:    Option<String>,
    pub objuuid:  Option<String>,
    pub coluuid:  Option<String>,
}

/// Request to retrieve all current peer connections.
/// Maps to Python's `GetPeers(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetPeers {
    #[serde(default)]
    pub peers:   Vec<Peer>,
    pub error:   Option<String>,
    pub objuuid: Option<String>,
    pub coluuid: Option<String>,
}

/// Request to retrieve the routing table.
/// Maps to Python's `GetRoutes(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetRoutes {
    #[serde(default)]
    pub routes:  Vec<Route>,
    pub error:   Option<String>,
    pub objuuid: Option<String>,
    pub coluuid: Option<String>,
}

/// Request to retrieve the agent configuration.
/// Maps to Python's `GetConfig(ControlForm)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetConfig {
    pub config:  Option<Value>,
    pub error:   Option<String>,
    pub objuuid: Option<String>,
    pub coluuid: Option<String>,
}

// ── Tagged union of all control form variants ─────────────────────────────────

/// Internally-tagged union of all control forms.
///
/// Serialises as `{ "type": "<type>", ...fields }`.
/// Tag values are lowercase to match Python's StrEnum wire format.
/// Used as the `form` field in `ControlFormTicket` and `NetworkTicket`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ControlFormVariant {
    #[serde(rename = "create_peer")]   CreatePeer(CreatePeer),
    #[serde(rename = "discover_peer")] DiscoverPeer(DiscoverPeer),
    #[serde(rename = "delete_peers")]  DeletePeers(DeletePeers),
    #[serde(rename = "get_peers")]     GetPeers(GetPeers),
    #[serde(rename = "get_routes")]    GetRoutes(GetRoutes),
    #[serde(rename = "sync_process")]  SyncProcess(SyncProcess),
    #[serde(rename = "write_file")]    WriteFile(WriteFile),
    #[serde(rename = "load_file")]     LoadFile(LoadFile),
    #[serde(rename = "get_config")]    GetConfig(GetConfig),
}

impl Default for ControlFormVariant {
    fn default() -> Self {
        Self::GetConfig(GetConfig::default())
    }
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlFormTicket {
    #[serde(rename = "type", default = "default_create_ticket")]
    pub form_type:    ControlFormType,
    #[serde(default = "gen_uuid")]
    pub tckuuid:      String,
    #[serde(default)]
    pub src:          String,
    #[serde(default)]
    pub dst:          String,
    #[serde(default = "unix_now_f64")]
    pub create_time:  f64,
    #[serde(default)]
    pub tracing:      bool,
    #[serde(default)]
    pub hops:         Vec<Hop>,
    pub form:         ControlFormVariant,
    pub service_time: Option<f64>,
    pub error:        Option<String>,
    pub objuuid:      Option<String>,
    pub coluuid:      Option<String>,
}

impl Default for ControlFormTicket {
    fn default() -> Self {
        Self {
            form_type:    ControlFormType::CreateTicket,
            tckuuid:      gen_uuid(),
            src:          String::new(),
            dst:          String::new(),
            create_time:  unix_now_f64(),
            tracing:      false,
            hops:         Vec::new(),
            form:         ControlFormVariant::default(),
            service_time: None,
            error:        None,
            objuuid:      None,
            coluuid:      None,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Compare serialized struct against a canonical JSON string (order-insensitive).
    /// Used to verify Rust output matches the Python protocol wire format.
    fn assert_ser_eq(value: &impl Serialize, expected_json: &str) {
        let got: serde_json::Value = serde_json::to_value(value).unwrap();
        let exp: serde_json::Value = serde_json::from_str(expected_json).unwrap();
        assert_eq!(got, exp);
    }

    /// Deserialize a canonical JSON string, re-serialize, and compare (round-trip).
    /// Proves the Rust types can consume and reproduce every protocol message.
    fn assert_deser_roundtrip<T>(json: &str)
    where
        T: serde::de::DeserializeOwned + Serialize,
    {
        let parsed: T = serde_json::from_str(json).unwrap();
        let got: serde_json::Value = serde_json::to_value(&parsed).unwrap();
        let exp: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(got, exp);
    }

    // ── LoadFile ──────────────────────────────────────────────────────────────

    const LOAD_FILE_REQUEST_JSON: &str = concat!(
        r#"{"type":"load_file","error":null,"objuuid":null,"coluuid":null,"#,
        r#""b64zlib":null,"path":"/etc/hosts","size":null,"md5sum":null}"#
    );
    const LOAD_FILE_RESPONSE_JSON: &str = concat!(
        r#"{"type":"load_file","error":null,"objuuid":null,"coluuid":null,"#,
        r#""b64zlib":"abc123","path":"/etc/hosts","size":1024,"#,
        r#""md5sum":"d8e8fca2dc0f896fd7cb4cb0031ba249"}"#
    );

    #[test]
    fn test_ser_load_file_request() {
        let form = ControlFormVariant::LoadFile(LoadFile {
            path: "/etc/hosts".into(),
            ..Default::default()
        });
        assert_ser_eq(&form, LOAD_FILE_REQUEST_JSON);
    }

    #[test]
    fn test_ser_load_file_response() {
        let form = ControlFormVariant::LoadFile(LoadFile {
            path: "/etc/hosts".into(),
            b64zlib: Some("abc123".into()),
            size: Some(1024),
            md5sum: Some("d8e8fca2dc0f896fd7cb4cb0031ba249".into()),
            ..Default::default()
        });
        assert_ser_eq(&form, LOAD_FILE_RESPONSE_JSON);
    }

    #[test]
    fn test_deser_load_file_request() {
        assert_deser_roundtrip::<ControlFormVariant>(LOAD_FILE_REQUEST_JSON);
    }

    #[test]
    fn test_deser_load_file_response() {
        assert_deser_roundtrip::<ControlFormVariant>(LOAD_FILE_RESPONSE_JSON);
    }

    // ── WriteFile ─────────────────────────────────────────────────────────────

    const WRITE_FILE_REQUEST_JSON: &str = concat!(
        r#"{"type":"write_file","error":null,"objuuid":null,"coluuid":null,"#,
        r#""b64zlib":"abc123","path":"/tmp/out.txt","size":null,"md5sum":null}"#
    );
    const WRITE_FILE_RESPONSE_JSON: &str = concat!(
        r#"{"type":"write_file","error":null,"objuuid":null,"coluuid":null,"#,
        r#""b64zlib":"abc123","path":"/tmp/out.txt","size":6,"#,
        r#""md5sum":"d8e8fca2dc0f896fd7cb4cb0031ba249"}"#
    );

    #[test]
    fn test_ser_write_file_request() {
        let form = ControlFormVariant::WriteFile(WriteFile {
            b64zlib: "abc123".into(),
            path: "/tmp/out.txt".into(),
            ..Default::default()
        });
        assert_ser_eq(&form, WRITE_FILE_REQUEST_JSON);
    }

    #[test]
    fn test_ser_write_file_response() {
        let form = ControlFormVariant::WriteFile(WriteFile {
            b64zlib: "abc123".into(),
            path: "/tmp/out.txt".into(),
            size: Some(6),
            md5sum: Some("d8e8fca2dc0f896fd7cb4cb0031ba249".into()),
            ..Default::default()
        });
        assert_ser_eq(&form, WRITE_FILE_RESPONSE_JSON);
    }

    #[test]
    fn test_deser_write_file_request() {
        assert_deser_roundtrip::<ControlFormVariant>(WRITE_FILE_REQUEST_JSON);
    }

    #[test]
    fn test_deser_write_file_response() {
        assert_deser_roundtrip::<ControlFormVariant>(WRITE_FILE_RESPONSE_JSON);
    }

    // ── SyncProcess ───────────────────────────────────────────────────────────

    const SYNC_PROCESS_STR_CMD_JSON: &str = concat!(
        r#"{"type":"sync_process","error":null,"objuuid":null,"coluuid":null,"#,
        r#""timeout":15,"command":"ls /","stdout":null,"stderr":null,"#,
        r#""status":null,"start_time":null,"elapsed_time":null}"#
    );
    const SYNC_PROCESS_LIST_CMD_JSON: &str = concat!(
        r#"{"type":"sync_process","error":null,"objuuid":null,"coluuid":null,"#,
        r#""timeout":15,"command":["ls","/"],"stdout":null,"stderr":null,"#,
        r#""status":null,"start_time":null,"elapsed_time":null}"#
    );
    const SYNC_PROCESS_RESPONSE_JSON: &str = concat!(
        r#"{"type":"sync_process","error":null,"objuuid":null,"coluuid":null,"#,
        r#""timeout":15,"command":"ls /","stdout":"bin\nboot\n","stderr":""#,
        r#"","status":0,"start_time":1000.0,"elapsed_time":0.01}"#
    );

    #[test]
    fn test_ser_sync_process_str_command() {
        let form = ControlFormVariant::SyncProcess(SyncProcess {
            command: CommandArg::Single("ls /".into()),
            timeout: 15,
            stdout: None, stderr: None, status: None,
            start_time: None, elapsed_time: None,
            error: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&form, SYNC_PROCESS_STR_CMD_JSON);
    }

    #[test]
    fn test_ser_sync_process_list_command() {
        let form = ControlFormVariant::SyncProcess(SyncProcess {
            command: CommandArg::Multi(vec!["ls".into(), "/".into()]),
            timeout: 15,
            stdout: None, stderr: None, status: None,
            start_time: None, elapsed_time: None,
            error: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&form, SYNC_PROCESS_LIST_CMD_JSON);
    }

    #[test]
    fn test_ser_sync_process_response() {
        let form = ControlFormVariant::SyncProcess(SyncProcess {
            command: CommandArg::Single("ls /".into()),
            timeout: 15,
            stdout: Some("bin\nboot\n".into()),
            stderr: Some("".into()),
            status: Some(0),
            start_time: Some(1000.0),
            elapsed_time: Some(0.01),
            error: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&form, SYNC_PROCESS_RESPONSE_JSON);
    }

    #[test]
    fn test_deser_sync_process_str_command() {
        assert_deser_roundtrip::<ControlFormVariant>(SYNC_PROCESS_STR_CMD_JSON);
    }

    #[test]
    fn test_deser_sync_process_list_command() {
        assert_deser_roundtrip::<ControlFormVariant>(SYNC_PROCESS_LIST_CMD_JSON);
    }

    #[test]
    fn test_deser_sync_process_response() {
        assert_deser_roundtrip::<ControlFormVariant>(SYNC_PROCESS_RESPONSE_JSON);
    }

    // ── CreatePeer ────────────────────────────────────────────────────────────

    // Python's HttpUrl normalises by appending a trailing slash
    const CREATE_PEER_JSON: &str = concat!(
        r#"{"type":"create_peer","error":null,"objuuid":null,"coluuid":null,"#,
        r#""url":"http://10.0.0.1:8080/","ttl":null,"polling":false,"agtuuid":"a1"}"#
    );

    #[test]
    fn test_ser_create_peer() {
        let form = ControlFormVariant::CreatePeer(CreatePeer {
            agtuuid: "a1".into(),
            url: Some("http://10.0.0.1:8080/".into()),
            ..Default::default()
        });
        assert_ser_eq(&form, CREATE_PEER_JSON);
    }

    #[test]
    fn test_deser_create_peer() {
        assert_deser_roundtrip::<ControlFormVariant>(CREATE_PEER_JSON);
    }

    // ── DiscoverPeer ──────────────────────────────────────────────────────────

    const DISCOVER_PEER_JSON: &str = concat!(
        r#"{"type":"discover_peer","error":null,"objuuid":null,"coluuid":null,"#,
        r#""agtuuid":null,"url":"http://10.0.0.1:8080","ttl":null,"polling":false}"#
    );

    #[test]
    fn test_ser_discover_peer() {
        let form = ControlFormVariant::DiscoverPeer(DiscoverPeer {
            url: "http://10.0.0.1:8080".into(),
            ..Default::default()
        });
        assert_ser_eq(&form, DISCOVER_PEER_JSON);
    }

    #[test]
    fn test_deser_discover_peer() {
        assert_deser_roundtrip::<ControlFormVariant>(DISCOVER_PEER_JSON);
    }

    // ── DeletePeers ───────────────────────────────────────────────────────────

    const DELETE_PEERS_JSON: &str = concat!(
        r#"{"type":"delete_peers","error":null,"objuuid":null,"coluuid":null,"#,
        r#""agtuuids":["a1","a2"]}"#
    );
    const DELETE_PEERS_ALL_JSON: &str = concat!(
        r#"{"type":"delete_peers","error":null,"objuuid":null,"coluuid":null,"#,
        r#""agtuuids":null}"#
    );

    #[test]
    fn test_ser_delete_peers() {
        let form = ControlFormVariant::DeletePeers(DeletePeers {
            agtuuids: Some(vec!["a1".into(), "a2".into()]),
            ..Default::default()
        });
        assert_ser_eq(&form, DELETE_PEERS_JSON);
    }

    #[test]
    fn test_ser_delete_peers_all() {
        let form = ControlFormVariant::DeletePeers(DeletePeers {
            agtuuids: None,
            ..Default::default()
        });
        assert_ser_eq(&form, DELETE_PEERS_ALL_JSON);
    }

    #[test]
    fn test_deser_delete_peers() {
        assert_deser_roundtrip::<ControlFormVariant>(DELETE_PEERS_JSON);
    }

    #[test]
    fn test_deser_delete_peers_all() {
        assert_deser_roundtrip::<ControlFormVariant>(DELETE_PEERS_ALL_JSON);
    }

    // ── GetPeers ──────────────────────────────────────────────────────────────

    const GET_PEERS_EMPTY_JSON: &str = concat!(
        r#"{"type":"get_peers","error":null,"objuuid":null,"coluuid":null,"peers":[]}"#
    );
    const GET_PEERS_DATA_JSON: &str = concat!(
        r#"{"type":"get_peers","error":null,"objuuid":null,"coluuid":null,"#,
        r#""peers":[{"agtuuid":"a2","polling":false,"destroy_time":2000.0,"#,
        r#""refresh_time":1000.0,"url":"http://10.0.0.2:8080","objuuid":null,"coluuid":null}]}"#
    );

    #[test]
    fn test_ser_get_peers_empty() {
        let form = ControlFormVariant::GetPeers(GetPeers::default());
        assert_ser_eq(&form, GET_PEERS_EMPTY_JSON);
    }

    #[test]
    fn test_ser_get_peers_with_data() {
        use crate::models::routing::Peer;
        let form = ControlFormVariant::GetPeers(GetPeers {
            peers: vec![Peer {
                agtuuid: Some("a2".into()),
                polling: false,
                destroy_time: Some(2000.0),
                refresh_time: Some(1000.0),
                url: Some("http://10.0.0.2:8080".into()),
                objuuid: None,
                coluuid: None,
            }],
            ..Default::default()
        });
        assert_ser_eq(&form, GET_PEERS_DATA_JSON);
    }

    #[test]
    fn test_deser_get_peers_empty() {
        assert_deser_roundtrip::<ControlFormVariant>(GET_PEERS_EMPTY_JSON);
    }

    #[test]
    fn test_deser_get_peers_with_data() {
        assert_deser_roundtrip::<ControlFormVariant>(GET_PEERS_DATA_JSON);
    }

    // ── GetRoutes ─────────────────────────────────────────────────────────────

    const GET_ROUTES_EMPTY_JSON: &str = concat!(
        r#"{"type":"get_routes","error":null,"objuuid":null,"coluuid":null,"routes":[]}"#
    );
    const GET_ROUTES_DATA_JSON: &str = concat!(
        r#"{"type":"get_routes","error":null,"objuuid":null,"coluuid":null,"#,
        r#""routes":[{"agtuuid":"a2","gtwuuid":"a1","weight":1,"objuuid":null,"coluuid":null}]}"#
    );

    #[test]
    fn test_ser_get_routes_empty() {
        let form = ControlFormVariant::GetRoutes(GetRoutes::default());
        assert_ser_eq(&form, GET_ROUTES_EMPTY_JSON);
    }

    #[test]
    fn test_ser_get_routes_with_data() {
        use crate::models::routing::Route;
        let form = ControlFormVariant::GetRoutes(GetRoutes {
            routes: vec![Route { agtuuid: "a2".into(), gtwuuid: "a1".into(), weight: 1, objuuid: None, coluuid: None }],
            ..Default::default()
        });
        assert_ser_eq(&form, GET_ROUTES_DATA_JSON);
    }

    #[test]
    fn test_deser_get_routes_empty() {
        assert_deser_roundtrip::<ControlFormVariant>(GET_ROUTES_EMPTY_JSON);
    }

    #[test]
    fn test_deser_get_routes_with_data() {
        assert_deser_roundtrip::<ControlFormVariant>(GET_ROUTES_DATA_JSON);
    }

    // ── GetConfig ─────────────────────────────────────────────────────────────

    const GET_CONFIG_REQUEST_JSON: &str = concat!(
        r#"{"type":"get_config","error":null,"objuuid":null,"coluuid":null,"config":null}"#
    );
    const GET_CONFIG_RESPONSE_JSON: &str = concat!(
        r#"{"type":"get_config","error":null,"objuuid":null,"coluuid":null,"#,
        r#""config":{"agtuuid":"a1","port":8080}}"#
    );

    #[test]
    fn test_ser_get_config_request() {
        let form = ControlFormVariant::GetConfig(GetConfig::default());
        assert_ser_eq(&form, GET_CONFIG_REQUEST_JSON);
    }

    #[test]
    fn test_ser_get_config_response() {
        let form = ControlFormVariant::GetConfig(GetConfig {
            config: Some(serde_json::json!({"agtuuid": "a1", "port": 8080})),
            ..Default::default()
        });
        assert_ser_eq(&form, GET_CONFIG_RESPONSE_JSON);
    }

    #[test]
    fn test_deser_get_config_request() {
        assert_deser_roundtrip::<ControlFormVariant>(GET_CONFIG_REQUEST_JSON);
    }

    #[test]
    fn test_deser_get_config_response() {
        assert_deser_roundtrip::<ControlFormVariant>(GET_CONFIG_RESPONSE_JSON);
    }

    // ── Hop ───────────────────────────────────────────────────────────────────

    const HOP_JSON: &str =
        r#"{"agtuuid":"a1","hop_time":1000.0,"type_str":"ticket_request"}"#;

    #[test]
    fn test_ser_hop() {
        let hop = Hop { agtuuid: "a1".into(), hop_time: 1000.0, type_str: "ticket_request".into() };
        assert_ser_eq(&hop, HOP_JSON);
    }

    #[test]
    fn test_deser_hop() {
        assert_deser_roundtrip::<Hop>(HOP_JSON);
    }

    // ── ControlFormTicket ─────────────────────────────────────────────────────

    const CFT_CREATE_JSON: &str = concat!(
        r#"{"type":"create_ticket","error":null,"objuuid":null,"coluuid":null,"#,
        r#""tckuuid":"t1","src":"a1","dst":"a2","create_time":1000.0,"#,
        r#""service_time":null,"tracing":false,"hops":[],"#,
        r#""form":{"type":"sync_process","error":null,"objuuid":null,"coluuid":null,"#,
        r#""timeout":15,"command":"ls /","stdout":null,"stderr":null,"#,
        r#""status":null,"start_time":null,"elapsed_time":null}}"#
    );
    const CFT_CLOSE_WITH_HOPS_JSON: &str = concat!(
        r#"{"type":"close_ticket","error":null,"objuuid":null,"coluuid":null,"#,
        r#""tckuuid":"t1","src":"a1","dst":"a2","create_time":1000.0,"#,
        r#""service_time":0.5,"tracing":true,"#,
        r#""hops":[{"agtuuid":"a1","hop_time":1001.0,"type_str":"ticket_request"}],"#,
        r#""form":{"type":"sync_process","error":null,"objuuid":null,"coluuid":null,"#,
        r#""timeout":15,"command":"ls /","stdout":null,"stderr":null,"#,
        r#""status":null,"start_time":null,"elapsed_time":null}}"#
    );

    fn sync_process_ls() -> ControlFormVariant {
        ControlFormVariant::SyncProcess(SyncProcess {
            command: CommandArg::Single("ls /".into()),
            timeout: 15,
            stdout: None, stderr: None, status: None,
            start_time: None, elapsed_time: None,
            error: None, objuuid: None, coluuid: None,
        })
    }

    #[test]
    fn test_ser_control_form_ticket_create() {
        let ticket = ControlFormTicket {
            form_type: ControlFormType::CreateTicket,
            tckuuid: "t1".into(),
            src: "a1".into(),
            dst: "a2".into(),
            create_time: 1000.0,
            service_time: None,
            tracing: false,
            hops: vec![],
            form: sync_process_ls(),
            error: None, objuuid: None, coluuid: None,
        };
        assert_ser_eq(&ticket, CFT_CREATE_JSON);
    }

    #[test]
    fn test_ser_control_form_ticket_close_with_hops() {
        let ticket = ControlFormTicket {
            form_type: ControlFormType::CloseTicket,
            tckuuid: "t1".into(),
            src: "a1".into(),
            dst: "a2".into(),
            create_time: 1000.0,
            service_time: Some(0.5),
            tracing: true,
            hops: vec![Hop { agtuuid: "a1".into(), hop_time: 1001.0, type_str: "ticket_request".into() }],
            form: sync_process_ls(),
            error: None, objuuid: None, coluuid: None,
        };
        assert_ser_eq(&ticket, CFT_CLOSE_WITH_HOPS_JSON);
    }

    #[test]
    fn test_deser_control_form_ticket_create() {
        assert_deser_roundtrip::<ControlFormTicket>(CFT_CREATE_JSON);
    }

    #[test]
    fn test_deser_control_form_ticket_close_with_hops() {
        assert_deser_roundtrip::<ControlFormTicket>(CFT_CLOSE_WITH_HOPS_JSON);
    }
}

