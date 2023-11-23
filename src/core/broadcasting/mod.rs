use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Broadcast {}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BroadcastRequest {
    origin_id: Option<String>,
    request_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BroadcastResponse {
    origin_id: String,
    destination_id: String,
    request_id: String,
}
