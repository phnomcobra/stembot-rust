use serde::{Deserialize, Serialize};
use crate::routing::Route;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteAdvertisement {
    pub routes: Vec<Route>,
}

pub enum MessageType {
    RouteAdvertisement(RouteAdvertisement)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub destination_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MessageCollection {
    pub messages: Vec<Message>,
    pub origin_id: String,         
}