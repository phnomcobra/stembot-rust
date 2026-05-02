use serde::{Deserialize, Serialize};

use crate::enums::NetworkMessageType;
use crate::models::control::{ControlFormVariant, Hop};
use crate::models::routing::Route;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn gen_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn unix_now() -> Option<f64> {
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
// These structs do NOT carry a `type` field; the type is encoded by the
// `NetworkMessageVariant` tagged enum when serialised.

/// Simple connectivity check message.
/// Maps to Python's `Ping(NetworkMessage)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Ping {
    #[serde(default)]
    pub src: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isrc: Option<String>,
    #[serde(default = "unix_now", skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// Request to retrieve pending messages from an agent.
/// Maps to Python's `NetworkMessagesRequest(NetworkMessage)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkMessagesRequest {
    #[serde(default)]
    pub src: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isrc: Option<String>,
    #[serde(default = "unix_now", skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// Acknowledgement of a received message.
/// Maps to Python's `Acknowledgement(NetworkMessage)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Acknowledgement {
    pub ack_type: NetworkMessageType,
    #[serde(default)]
    pub src: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isrc: Option<String>,
    #[serde(default = "unix_now", skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forwarded: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// Advertisement of routes known by an agent.
/// Maps to Python's `Advertisement(NetworkMessage)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Advertisement {
    pub agtuuid: String,
    #[serde(default)]
    pub routes: Vec<Route>,
    #[serde(default)]
    pub src: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isrc: Option<String>,
    #[serde(default = "unix_now", skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// Response to a NetworkMessagesRequest containing pending messages.
/// Maps to Python's `NetworkMessagesResponse(NetworkMessage)`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkMessagesResponse {
    #[serde(default)]
    pub messages: Vec<NetworkMessageVariant>,
    #[serde(default)]
    pub src: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isrc: Option<String>,
    #[serde(default = "unix_now", skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// Response indicating a ticket has been traced through the network.
/// Maps to Python's `TicketTraceResponse(NetworkMessage)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketTraceResponse {
    pub tckuuid: String,
    pub network_ticket_type: NetworkMessageType,
    #[serde(default = "unix_now_f64")]
    pub hop_time: f64,
    #[serde(default)]
    pub src: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isrc: Option<String>,
    #[serde(default = "unix_now", skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
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
/// This struct is used for both `TICKET_REQUEST` and `TICKET_RESPONSE`
/// variants in `NetworkMessageVariant` — the type is encoded by the variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkTicket {
    #[serde(default = "gen_uuid")]
    pub tckuuid: String,
    pub form: ControlFormVariant,
    #[serde(default)]
    pub tracing: bool,
    #[serde(default)]
    pub src: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isrc: Option<String>,
    #[serde(default = "unix_now", skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

// ── Tagged union of all network message variants ──────────────────────────────

/// Internally-tagged union of all network message types.
///
/// Serialises as `{ "type": "<TYPE>", ...fields }`.
/// Used as the element type of `NetworkMessagesResponse::messages`.
/// Maps to Python's `NetworkMessage` base class used as `List[NetworkMessage]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NetworkMessageVariant {
    #[serde(rename = "PING")]                  Ping(Ping),
    #[serde(rename = "MESSAGES_REQUEST")]      MessagesRequest(NetworkMessagesRequest),
    #[serde(rename = "MESSAGES_RESPONSE")]     MessagesResponse(NetworkMessagesResponse),
    #[serde(rename = "ACKNOWLEDGEMENT")]       Acknowledgement(Acknowledgement),
    #[serde(rename = "ADVERTISEMENT")]         Advertisement(Advertisement),
    #[serde(rename = "TICKET_TRACE_RESPONSE")] TicketTraceResponse(TicketTraceResponse),
    #[serde(rename = "TICKET_REQUEST")]        TicketRequest(NetworkTicket),
    #[serde(rename = "TICKET_RESPONSE")]       TicketResponse(NetworkTicket),
}
