use crate::{io::http::client::send_raw_message, routing::Route, state::Singleton};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::{self, Display},
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteAdvertisement {
    pub routes: Vec<Route>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteRecall {
    pub destination_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BacklogResponse {
    pub message_collections: Vec<MessageCollection>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BacklogRequest {
    pub gateway_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TraceRequest {
    pub hop_count: usize,
    pub request_id: String,
    pub start_time: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TraceResponse {
    pub hop_count: usize,
    pub request_id: String,
    pub start_time: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Direction {
    Outbound,
    Inbound,
}

impl Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Inbound => write!(f, "inbound"),
            Self::Outbound => write!(f, "outbound"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TraceEvent {
    pub hop_count: usize,
    pub request_id: String,
    pub local_time: u128,
    pub id: String,
    pub direction: Direction,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Ticket {
    Test,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketRequest {
    pub ticket: Ticket,
    pub ticket_id: String,
    pub start_time: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketResponse {
    pub ticket: Ticket,
    pub ticket_id: String,
    pub start_time: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Message {
    RouteAdvertisement(RouteAdvertisement),
    RouteRecall(RouteRecall),
    BacklogResponse(BacklogResponse),
    BacklogRequest(BacklogRequest),
    Ping,
    Pong,
    TraceRequest(TraceRequest),
    TraceResponse(TraceResponse),
    TraceEvent(TraceEvent),
    TicketRequest(TicketRequest),
    TicketResponse(TicketResponse),
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

pub async fn send_message_collection_to_url(
    outgoing_message_collection: MessageCollection,
    url: String,
    singleton: Singleton,
) -> Result<MessageCollection, Box<dyn Error + Send + Sync + 'static>> {
    let outgoing_message_bytes: Vec<u8> = match outgoing_message_collection.try_into() {
        Ok(collection) => collection,
        Err(error) => return Err(error),
    };

    match send_raw_message(outgoing_message_bytes, url, singleton.configuration).await {
        Ok(bytes) => {
            let incoming_message_collection: MessageCollection = match bytes.try_into() {
                Ok(collection) => collection,
                Err(error) => return Err(error),
            };

            Ok(incoming_message_collection)
        }
        Err(error) => Err(error),
    }
}
