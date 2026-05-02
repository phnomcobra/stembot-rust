use serde::{Deserialize, Serialize};

use crate::enums::NetworkMessageType;
use crate::models::control::{ControlFormVariant, Hop};
use crate::models::routing::Route;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn gen_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn unix_now_opt() -> Option<f64> {
    Some(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64(),
    )
}

fn unix_now_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

// ── Individual network message structs ───────────────────────────────────────
// These structs do NOT carry a `type` field; it is encoded by the
// `NetworkMessageVariant` tagged enum when serialised.
// All Option fields serialize as null (no skip_serializing_if) to match
// the Python protocol wire format.

/// Simple connectivity check message.
/// Maps to Python's `Ping(NetworkMessage)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Ping {
    #[serde(default)]
    pub src:       String,
    pub dest:      Option<String>,
    pub isrc:      Option<String>,
    #[serde(default = "unix_now_opt")]
    pub timestamp: Option<f64>,
    pub objuuid:   Option<String>,
    pub coluuid:   Option<String>,
}

/// Request to retrieve pending messages from an agent.
/// Maps to Python's `NetworkMessagesRequest(NetworkMessage)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkMessagesRequest {
    #[serde(default)]
    pub src:       String,
    pub dest:      Option<String>,
    pub isrc:      Option<String>,
    #[serde(default = "unix_now_opt")]
    pub timestamp: Option<f64>,
    pub objuuid:   Option<String>,
    pub coluuid:   Option<String>,
}

/// Acknowledgement of a received message.
/// Maps to Python's `Acknowledgement(NetworkMessage)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Acknowledgement {
    pub ack_type:  NetworkMessageType,
    #[serde(default)]
    pub src:       String,
    pub dest:      Option<String>,
    pub isrc:      Option<String>,
    #[serde(default = "unix_now_opt")]
    pub timestamp: Option<f64>,
    pub forwarded: Option<String>,
    pub error:     Option<String>,
    pub objuuid:   Option<String>,
    pub coluuid:   Option<String>,
}

/// Advertisement of routes known by an agent.
/// Maps to Python's `Advertisement(NetworkMessage)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Advertisement {
    pub agtuuid:   String,
    #[serde(default)]
    pub routes:    Vec<Route>,
    #[serde(default)]
    pub src:       String,
    pub dest:      Option<String>,
    pub isrc:      Option<String>,
    #[serde(default = "unix_now_opt")]
    pub timestamp: Option<f64>,
    pub objuuid:   Option<String>,
    pub coluuid:   Option<String>,
}

/// Response to a NetworkMessagesRequest containing pending messages.
/// Maps to Python's `NetworkMessagesResponse(NetworkMessage)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkMessagesResponse {
    #[serde(default)]
    pub messages:  Vec<NetworkMessageVariant>,
    #[serde(default)]
    pub src:       String,
    pub dest:      Option<String>,
    pub isrc:      Option<String>,
    #[serde(default = "unix_now_opt")]
    pub timestamp: Option<f64>,
    pub objuuid:   Option<String>,
    pub coluuid:   Option<String>,
}

/// Response indicating a ticket has been traced through the network.
/// Maps to Python's `TicketTraceResponse(NetworkMessage)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketTraceResponse {
    pub tckuuid:             String,
    pub network_ticket_type: NetworkMessageType,
    #[serde(default = "unix_now_f64")]
    pub hop_time:            f64,
    #[serde(default)]
    pub src:                 String,
    pub dest:                Option<String>,
    pub isrc:                Option<String>,
    #[serde(default = "unix_now_opt")]
    pub timestamp:           Option<f64>,
    pub objuuid:             Option<String>,
    pub coluuid:             Option<String>,
}

impl TicketTraceResponse {
    /// Returns a `Hop` representing this trace response.
    /// Maps to the `hop` property in Python's `TicketTraceResponse`.
    pub fn hop(&self) -> Hop {
        Hop {
            agtuuid:  self.src.clone(),
            hop_time: self.hop_time,
            type_str: self.network_ticket_type.to_string(),
        }
    }
}

/// A ticket for asynchronous message delivery across the network.
/// Maps to Python's `NetworkTicket(NetworkMessage)`.
///
/// Used for both `TICKET_REQUEST` and `TICKET_RESPONSE` variants in
/// `NetworkMessageVariant` — the type is encoded by the variant tag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkTicket {
    #[serde(default = "gen_uuid")]
    pub tckuuid:      String,
    pub form:         ControlFormVariant,
    #[serde(default)]
    pub tracing:      bool,
    #[serde(default)]
    pub src:          String,
    pub dest:         Option<String>,
    pub isrc:         Option<String>,
    #[serde(default = "unix_now_opt")]
    pub timestamp:    Option<f64>,
    pub create_time:  Option<f64>,
    pub service_time: Option<f64>,
    pub error:        Option<String>,
    pub objuuid:      Option<String>,
    pub coluuid:      Option<String>,
}

// ── Tagged union of all network message variants ──────────────────────────────

/// Internally-tagged union of all network message types.
///
/// Serialises as `{ "type": "<type>", ...fields }`.
/// Tag values are lowercase to match Python's StrEnum wire format.
/// Used as the element type of `NetworkMessagesResponse::messages`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NetworkMessageVariant {
    #[serde(rename = "ping")]                  Ping(Ping),
    #[serde(rename = "messages_request")]      MessagesRequest(NetworkMessagesRequest),
    #[serde(rename = "messages_response")]     MessagesResponse(NetworkMessagesResponse),
    #[serde(rename = "acknowledgement")]       Acknowledgement(Acknowledgement),
    #[serde(rename = "advertisement")]         Advertisement(Advertisement),
    #[serde(rename = "ticket_trace_response")] TicketTraceResponse(TicketTraceResponse),
    #[serde(rename = "ticket_request")]        TicketRequest(NetworkTicket),
    #[serde(rename = "ticket_response")]       TicketResponse(NetworkTicket),
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::control::{CommandArg, SyncProcess};

    fn assert_ser_eq(value: &impl Serialize, expected_json: &str) {
        let got: serde_json::Value = serde_json::to_value(value).unwrap();
        let exp: serde_json::Value = serde_json::from_str(expected_json).unwrap();
        assert_eq!(got, exp);
    }

    fn assert_deser_roundtrip<T>(json: &str)
    where
        T: serde::de::DeserializeOwned + Serialize,
    {
        let parsed: T = serde_json::from_str(json).unwrap();
        let got: serde_json::Value = serde_json::to_value(&parsed).unwrap();
        let exp: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(got, exp);
    }

    // ── Ping ──────────────────────────────────────────────────────────────────

    const PING_JSON: &str = concat!(
        r#"{"type":"ping","dest":null,"src":"a1","isrc":null,"timestamp":1000.0,"#,
        r#""objuuid":null,"coluuid":null}"#
    );

    #[test]
    fn test_ser_ping() {
        let msg = NetworkMessageVariant::Ping(Ping {
            src: "a1".into(),
            timestamp: Some(1000.0),
            dest: None, isrc: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&msg, PING_JSON);
    }

    #[test]
    fn test_deser_ping() {
        assert_deser_roundtrip::<NetworkMessageVariant>(PING_JSON);
    }

    // ── NetworkMessagesRequest ────────────────────────────────────────────────

    const MSGS_REQUEST_JSON: &str = concat!(
        r#"{"type":"messages_request","dest":null,"src":"a1","isrc":null,"timestamp":1000.0,"#,
        r#""objuuid":null,"coluuid":null}"#
    );

    #[test]
    fn test_ser_network_messages_request() {
        let msg = NetworkMessageVariant::MessagesRequest(NetworkMessagesRequest {
            src: "a1".into(),
            timestamp: Some(1000.0),
            dest: None, isrc: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&msg, MSGS_REQUEST_JSON);
    }

    #[test]
    fn test_deser_network_messages_request() {
        assert_deser_roundtrip::<NetworkMessageVariant>(MSGS_REQUEST_JSON);
    }

    // ── Acknowledgement ───────────────────────────────────────────────────────

    const ACK_PING_JSON: &str = concat!(
        r#"{"type":"acknowledgement","dest":null,"src":"a1","isrc":null,"timestamp":1000.0,"#,
        r#""objuuid":null,"coluuid":null,"ack_type":"ping","forwarded":null,"error":null}"#
    );
    const ACK_ERROR_JSON: &str = concat!(
        r#"{"type":"acknowledgement","dest":null,"src":"a1","isrc":null,"timestamp":1000.0,"#,
        r#""objuuid":null,"coluuid":null,"ack_type":"ticket_request","forwarded":null,"error":"timeout"}"#
    );
    const ACK_FORWARDED_JSON: &str = concat!(
        r#"{"type":"acknowledgement","dest":null,"src":"a1","isrc":null,"timestamp":1000.0,"#,
        r#""objuuid":null,"coluuid":null,"ack_type":"ping","forwarded":"a2","error":null}"#
    );

    #[test]
    fn test_ser_acknowledgement_ping() {
        let msg = NetworkMessageVariant::Acknowledgement(Acknowledgement {
            ack_type: NetworkMessageType::Ping,
            src: "a1".into(),
            timestamp: Some(1000.0),
            dest: None, isrc: None, forwarded: None, error: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&msg, ACK_PING_JSON);
    }

    #[test]
    fn test_ser_acknowledgement_with_error() {
        let msg = NetworkMessageVariant::Acknowledgement(Acknowledgement {
            ack_type: NetworkMessageType::TicketRequest,
            src: "a1".into(),
            timestamp: Some(1000.0),
            error: Some("timeout".into()),
            dest: None, isrc: None, forwarded: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&msg, ACK_ERROR_JSON);
    }

    #[test]
    fn test_ser_acknowledgement_forwarded() {
        let msg = NetworkMessageVariant::Acknowledgement(Acknowledgement {
            ack_type: NetworkMessageType::Ping,
            src: "a1".into(),
            timestamp: Some(1000.0),
            forwarded: Some("a2".into()),
            dest: None, isrc: None, error: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&msg, ACK_FORWARDED_JSON);
    }

    #[test]
    fn test_deser_acknowledgement_ping() {
        assert_deser_roundtrip::<NetworkMessageVariant>(ACK_PING_JSON);
    }

    #[test]
    fn test_deser_acknowledgement_with_error() {
        assert_deser_roundtrip::<NetworkMessageVariant>(ACK_ERROR_JSON);
    }

    #[test]
    fn test_deser_acknowledgement_forwarded() {
        assert_deser_roundtrip::<NetworkMessageVariant>(ACK_FORWARDED_JSON);
    }

    // ── Advertisement ─────────────────────────────────────────────────────────

    const ADV_EMPTY_JSON: &str = concat!(
        r#"{"type":"advertisement","dest":null,"src":"a1","isrc":null,"timestamp":1000.0,"#,
        r#""objuuid":null,"coluuid":null,"routes":[],"agtuuid":"a1"}"#
    );
    const ADV_ROUTES_JSON: &str = concat!(
        r#"{"type":"advertisement","dest":null,"src":"a1","isrc":null,"timestamp":1000.0,"#,
        r#""objuuid":null,"coluuid":null,"#,
        r#""routes":[{"agtuuid":"a2","gtwuuid":"a1","weight":1,"objuuid":null,"coluuid":null}],"#,
        r#""agtuuid":"a1"}"#
    );

    #[test]
    fn test_ser_advertisement_empty_routes() {
        let msg = NetworkMessageVariant::Advertisement(Advertisement {
            agtuuid: "a1".into(),
            src: "a1".into(),
            timestamp: Some(1000.0),
            routes: vec![],
            dest: None, isrc: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&msg, ADV_EMPTY_JSON);
    }

    #[test]
    fn test_ser_advertisement_with_routes() {
        use crate::models::routing::Route;
        let msg = NetworkMessageVariant::Advertisement(Advertisement {
            agtuuid: "a1".into(),
            src: "a1".into(),
            timestamp: Some(1000.0),
            routes: vec![Route { agtuuid: "a2".into(), gtwuuid: "a1".into(), weight: 1, objuuid: None, coluuid: None }],
            dest: None, isrc: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&msg, ADV_ROUTES_JSON);
    }

    #[test]
    fn test_deser_advertisement_empty_routes() {
        assert_deser_roundtrip::<NetworkMessageVariant>(ADV_EMPTY_JSON);
    }

    #[test]
    fn test_deser_advertisement_with_routes() {
        assert_deser_roundtrip::<NetworkMessageVariant>(ADV_ROUTES_JSON);
    }

    // ── NetworkMessagesResponse ───────────────────────────────────────────────

    const MSGS_RESP_EMPTY_JSON: &str = concat!(
        r#"{"type":"messages_response","dest":null,"src":"a1","isrc":null,"timestamp":1000.0,"#,
        r#""objuuid":null,"coluuid":null,"messages":[]}"#
    );
    const MSGS_RESP_WITH_PING_JSON: &str = concat!(
        r#"{"type":"messages_response","dest":null,"src":"a1","isrc":null,"timestamp":1000.0,"#,
        r#""objuuid":null,"coluuid":null,"#,
        r#""messages":[{"type":"ping","dest":null,"src":"b1","isrc":null,"timestamp":2000.0,"#,
        r#""objuuid":null,"coluuid":null}]}"#
    );

    #[test]
    fn test_ser_network_messages_response_empty() {
        let msg = NetworkMessageVariant::MessagesResponse(NetworkMessagesResponse {
            src: "a1".into(),
            timestamp: Some(1000.0),
            messages: vec![],
            dest: None, isrc: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&msg, MSGS_RESP_EMPTY_JSON);
    }

    #[test]
    fn test_ser_network_messages_response_with_ping() {
        let msg = NetworkMessageVariant::MessagesResponse(NetworkMessagesResponse {
            src: "a1".into(),
            timestamp: Some(1000.0),
            messages: vec![NetworkMessageVariant::Ping(Ping {
                src: "b1".into(),
                timestamp: Some(2000.0),
                dest: None, isrc: None, objuuid: None, coluuid: None,
            })],
            dest: None, isrc: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&msg, MSGS_RESP_WITH_PING_JSON);
    }

    #[test]
    fn test_deser_network_messages_response_empty() {
        assert_deser_roundtrip::<NetworkMessageVariant>(MSGS_RESP_EMPTY_JSON);
    }

    #[test]
    fn test_deser_network_messages_response_with_ping() {
        assert_deser_roundtrip::<NetworkMessageVariant>(MSGS_RESP_WITH_PING_JSON);
    }

    // ── TicketTraceResponse ───────────────────────────────────────────────────

    const TTR_JSON: &str = concat!(
        r#"{"type":"ticket_trace_response","dest":null,"src":"a1","isrc":null,"timestamp":1000.0,"#,
        r#""objuuid":null,"coluuid":null,"tckuuid":"t1","hop_time":1000.0,"#,
        r#""network_ticket_type":"ticket_request"}"#
    );

    #[test]
    fn test_ser_ticket_trace_response() {
        let msg = NetworkMessageVariant::TicketTraceResponse(TicketTraceResponse {
            tckuuid: "t1".into(),
            network_ticket_type: NetworkMessageType::TicketRequest,
            hop_time: 1000.0,
            src: "a1".into(),
            timestamp: Some(1000.0),
            dest: None, isrc: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&msg, TTR_JSON);
    }

    #[test]
    fn test_deser_ticket_trace_response() {
        assert_deser_roundtrip::<NetworkMessageVariant>(TTR_JSON);
    }

    // ── NetworkTicket ─────────────────────────────────────────────────────────

    const NT_REQUEST_JSON: &str = concat!(
        r#"{"type":"ticket_request","dest":null,"src":"a1","isrc":null,"timestamp":1000.0,"#,
        r#""objuuid":null,"coluuid":null,"tckuuid":"t1","error":null,"create_time":null,"#,
        r#""service_time":null,"tracing":false,"#,
        r#""form":{"type":"sync_process","error":null,"objuuid":null,"coluuid":null,"#,
        r#""timeout":15,"command":"ls /","stdout":null,"stderr":null,"#,
        r#""status":null,"start_time":null,"elapsed_time":null}}"#
    );
    const NT_RESPONSE_JSON: &str = concat!(
        r#"{"type":"ticket_response","dest":null,"src":"a1","isrc":null,"timestamp":1000.0,"#,
        r#""objuuid":null,"coluuid":null,"tckuuid":"t1","error":null,"create_time":null,"#,
        r#""service_time":0.5,"tracing":false,"#,
        r#""form":{"type":"sync_process","error":null,"objuuid":null,"coluuid":null,"#,
        r#""timeout":15,"command":"ls /","stdout":"bin\n","stderr":null,"#,
        r#""status":0,"start_time":1000.0,"elapsed_time":0.1}}"#
    );

    fn sync_ls_request() -> ControlFormVariant {
        ControlFormVariant::SyncProcess(SyncProcess {
            command: CommandArg::Single("ls /".into()),
            timeout: 15,
            stdout: None, stderr: None, status: None,
            start_time: None, elapsed_time: None,
            error: None, objuuid: None, coluuid: None,
        })
    }

    #[test]
    fn test_ser_network_ticket_request() {
        let msg = NetworkMessageVariant::TicketRequest(NetworkTicket {
            tckuuid: "t1".into(),
            src: "a1".into(),
            timestamp: Some(1000.0),
            form: sync_ls_request(),
            tracing: false,
            dest: None, isrc: None,
            create_time: None, service_time: None,
            error: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&msg, NT_REQUEST_JSON);
    }

    #[test]
    fn test_ser_network_ticket_response() {
        let msg = NetworkMessageVariant::TicketResponse(NetworkTicket {
            tckuuid: "t1".into(),
            src: "a1".into(),
            timestamp: Some(1000.0),
            service_time: Some(0.5),
            form: ControlFormVariant::SyncProcess(SyncProcess {
                command: CommandArg::Single("ls /".into()),
                timeout: 15,
                stdout: Some("bin\n".into()),
                stderr: None,
                status: Some(0),
                start_time: Some(1000.0),
                elapsed_time: Some(0.1),
                error: None, objuuid: None, coluuid: None,
            }),
            tracing: false,
            dest: None, isrc: None,
            create_time: None,
            error: None, objuuid: None, coluuid: None,
        });
        assert_ser_eq(&msg, NT_RESPONSE_JSON);
    }

    #[test]
    fn test_deser_network_ticket_request() {
        assert_deser_roundtrip::<NetworkMessageVariant>(NT_REQUEST_JSON);
    }

    #[test]
    fn test_deser_network_ticket_response() {
        assert_deser_roundtrip::<NetworkMessageVariant>(NT_RESPONSE_JSON);
    }
}

