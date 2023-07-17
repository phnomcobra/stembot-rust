use crate::{config::Configuration, io::http::client::send_raw_message, routing::Route};
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteAdvertisement {
    pub routes: Vec<Route>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteRecall {
    pub destination_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Message {
    RouteAdvertisement(RouteAdvertisement),
    RouteRecall(RouteRecall),
    Ping,
    Pong,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MessageCollection {
    pub messages: Vec<Message>,
    pub origin_id: String,
    pub destination_id: Option<String>,
}

impl TryFrom<Vec<u8>> for MessageCollection {
    type Error = Box<dyn Error + Send + Sync + 'static>;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Box<dyn Error + Send + Sync + 'static>> {
        match bincode::deserialize::<MessageCollection>(&bytes) {
            Ok(message_collection) => Ok(message_collection),
            Err(_) => Err("failed to deserialize message collection".into()),
        }
    }
}

impl TryInto<Vec<u8>> for MessageCollection {
    type Error = Box<dyn Error + Send + Sync + 'static>;

    fn try_into(self) -> Result<Vec<u8>, Box<dyn Error + Send + Sync + 'static>> {
        match bincode::serialize::<MessageCollection>(&self) {
            Ok(bytes) => Ok(bytes),
            Err(_) => Err("failed to serialize message collection".into()),
        }
    }
}

pub async fn send_message_collection_to_url<U: Into<String>, V: Into<Configuration>>(
    outgoing_message_collection: MessageCollection,
    url: U,
    configuration: V,
) -> Result<MessageCollection, Box<dyn Error + Send + Sync + 'static>> {
    let url = url.into();
    let configuration = configuration.into();

    let outgoing_message_bytes: Vec<u8> = match outgoing_message_collection.try_into() {
        Ok(collection) => collection,
        Err(error) => return Err(error.into()),
    };

    match send_raw_message(outgoing_message_bytes, url, configuration).await {
        Ok(bytes) => {
            let incoming_message_collection: MessageCollection = match bytes.try_into() {
                Ok(collection) => collection,
                Err(error) => return Err(error.into()),
            };

            Ok(incoming_message_collection)
        }
        Err(error) => Err(error),
    }
}
