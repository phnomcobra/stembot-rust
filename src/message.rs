use std::error::Error;

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

impl TryFrom<Vec<u8>> for MessageCollection {
    type Error = Box<dyn Error>;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Box<dyn Error>> {
        match bincode::deserialize::<MessageCollection>(&bytes) {
            Ok(message_collection) => Ok(message_collection),
            Err(_) => Err("failed to deserialize message collection".into()),
        }
    }
}

impl TryInto<Vec<u8>> for MessageCollection {
    type Error = Box<dyn Error>;

    fn try_into(self) -> Result<Vec<u8>, Box<dyn Error>> {
        match bincode::serialize::<MessageCollection>(&self) {
            Ok(bytes) => Ok(bytes),
            Err(_) => Err("failed to serialize message collection".into()),
        }
    }
}
