use serde::{Deserialize, Serialize};

/// A route to another agent through a gateway.
/// Maps to Python's `Route(BaseModel)` in `models/routing.py`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Route {
    pub agtuuid: String,
    pub gtwuuid: String,
    pub weight:  i64,
    pub objuuid: Option<String>,
    pub coluuid: Option<String>,
}

/// A peering relationship with another agent.
/// Maps to Python's `Peer(BaseModel)` in `models/routing.py`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Peer {
    pub agtuuid:      Option<String>,
    #[serde(default)]
    pub polling:      bool,
    pub destroy_time: Option<f64>,
    pub refresh_time: Option<f64>,
    pub url:          Option<String>,
    pub objuuid:      Option<String>,
    pub coluuid:      Option<String>,
}
