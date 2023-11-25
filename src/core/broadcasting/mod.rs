pub mod processing;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BroadcastMessage {
    Ping,
    Pong,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Broadcast {
    pub request: BroadcastRequest,
    pub responses: HashMap<String, BroadcastResponse>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BroadcastRequest {
    pub origin_id: Option<String>,
    pub request_id: String,
    pub broadcast_message: BroadcastMessage,
    // forwarding_tags: Vec<String>
    // executive_tags: Vec<String>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BroadcastResponse {
    pub request_id: String,
    pub broadcast: BroadcastMessage,
    pub local_time: u128,
}
