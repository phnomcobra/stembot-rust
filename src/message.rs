use serde::{Deserialize, Serialize};
use crate::routing::Route;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteAdvertisement {
    pub routes: Vec<Route>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MessageType {
    RouteAdvertisement(RouteAdvertisement),
    Ping,
    Pong,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub destination_id: Option<String>,
    pub origin_id: String,
    pub message: MessageType,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MessageCollection {
    pub messages: Vec<Message>,
    pub origin_id: String,         
}