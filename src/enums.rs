use serde::{Deserialize, Serialize};

/// Control form operation types.
/// Maps to Python's `ControlFormType(UpperCaseStrEnum)`.
/// Serde renames use lowercase to match Python's StrEnum wire format.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlFormType {
    #[serde(rename = "create_peer")]   CreatePeer,
    #[serde(rename = "discover_peer")] DiscoverPeer,
    #[serde(rename = "delete_peers")]  DeletePeers,
    #[serde(rename = "get_peers")]     GetPeers,
    #[serde(rename = "get_routes")]    GetRoutes,
    #[serde(rename = "sync_process")]  SyncProcess,
    #[serde(rename = "write_file")]    WriteFile,
    #[serde(rename = "load_file")]     LoadFile,
    #[default]
    #[serde(rename = "create_ticket")] CreateTicket,
    #[serde(rename = "read_ticket")]   ReadTicket,
    #[serde(rename = "delete_ticket")] DeleteTicket,
    #[serde(rename = "close_ticket")]  CloseTicket,
    #[serde(rename = "get_config")]    GetConfig,
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



