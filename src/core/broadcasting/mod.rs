pub mod processing;

use std::{collections::HashMap, fmt::Display};

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

impl Broadcast {
    pub fn new(
        broadcast_message: BroadcastMessage,
        request_id: Option<String>,
        origin_id: Option<String>,
    ) -> Broadcast {
        let request_id = if let Some(request_id) = request_id {
            request_id
        } else {
            rand::random::<usize>().to_string()
        };

        let request = BroadcastRequest {
            origin_id,
            request_id,
            broadcast_message,
        };

        Broadcast {
            request,
            responses: HashMap::default(),
        }
    }
}

impl Display for BroadcastResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {:?}", self.request_id, self.broadcast)
    }
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
