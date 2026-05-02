use serde::{Deserialize, Serialize};

/// A route to another agent through a gateway.
/// Maps to Python's `Route(BaseModel)` in `models/routing.py`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Route {
    pub agtuuid: String,
    pub gtwuuid: String,
    pub weight:  i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid: Option<String>,
}

/// A peering relationship with another agent.
/// Maps to Python's `Peer(BaseModel)` in `models/routing.py`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Peer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agtuuid:      Option<String>,
    #[serde(default)]
    pub polling:      bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destroy_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url:          Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objuuid:      Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coluuid:      Option<String>,
}
