use crate::routing::Route;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteAdvertisement {
    pub routes: Vec<Route>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Message {
    RouteAdvertisement(RouteAdvertisement),
    Ping,
    Pong,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MessageCollection {
    pub messages: Vec<Message>,
    pub origin_id: String,
}
