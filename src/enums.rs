use serde::{Deserialize, Serialize};

/// Control form operation types.
/// Maps to Python's `ControlFormType(UpperCaseStrEnum)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlFormType {
    #[serde(rename = "CREATE_PEER")]   CreatePeer,
    #[serde(rename = "DISCOVER_PEER")] DiscoverPeer,
    #[serde(rename = "DELETE_PEERS")]  DeletePeers,
    #[serde(rename = "GET_PEERS")]     GetPeers,
    #[serde(rename = "GET_ROUTES")]    GetRoutes,
    #[serde(rename = "SYNC_PROCESS")]  SyncProcess,
    #[serde(rename = "WRITE_FILE")]    WriteFile,
    #[serde(rename = "LOAD_FILE")]     LoadFile,
    #[serde(rename = "CREATE_TICKET")] CreateTicket,
    #[serde(rename = "READ_TICKET")]   ReadTicket,
    #[serde(rename = "DELETE_TICKET")] DeleteTicket,
    #[serde(rename = "CLOSE_TICKET")]  CloseTicket,
    #[serde(rename = "GET_CONFIG")]    GetConfig,
}

impl std::fmt::Display for ControlFormType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::CreatePeer   => "CREATE_PEER",
            Self::DiscoverPeer => "DISCOVER_PEER",
            Self::DeletePeers  => "DELETE_PEERS",
            Self::GetPeers     => "GET_PEERS",
            Self::GetRoutes    => "GET_ROUTES",
            Self::SyncProcess  => "SYNC_PROCESS",
            Self::WriteFile    => "WRITE_FILE",
            Self::LoadFile     => "LOAD_FILE",
            Self::CreateTicket => "CREATE_TICKET",
            Self::ReadTicket   => "READ_TICKET",
            Self::DeleteTicket => "DELETE_TICKET",
            Self::CloseTicket  => "CLOSE_TICKET",
            Self::GetConfig    => "GET_CONFIG",
        };
        write!(f, "{s}")
    }
}

/// Network message types.
/// Maps to Python's `NetworkMessageType(UpperCaseStrEnum)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkMessageType {
    #[serde(rename = "ADVERTISEMENT")]         Advertisement,
    #[serde(rename = "MESSAGES_REQUEST")]      MessagesRequest,
    #[serde(rename = "MESSAGES_RESPONSE")]     MessagesResponse,
    #[serde(rename = "TICKET_REQUEST")]        TicketRequest,
    #[serde(rename = "TICKET_RESPONSE")]       TicketResponse,
    #[serde(rename = "TICKET_TRACE_RESPONSE")] TicketTraceResponse,
    #[serde(rename = "PING")]                  Ping,
    #[serde(rename = "ACKNOWLEDGEMENT")]       Acknowledgement,
}

impl std::fmt::Display for NetworkMessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Advertisement       => "ADVERTISEMENT",
            Self::MessagesRequest     => "MESSAGES_REQUEST",
            Self::MessagesResponse    => "MESSAGES_RESPONSE",
            Self::TicketRequest       => "TICKET_REQUEST",
            Self::TicketResponse      => "TICKET_RESPONSE",
            Self::TicketTraceResponse => "TICKET_TRACE_RESPONSE",
            Self::Ping                => "PING",
            Self::Acknowledgement     => "ACKNOWLEDGEMENT",
        };
        write!(f, "{s}")
    }
}
