pub mod processing;
pub mod promiscuous_decoding;
pub mod selective_decoding;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::core::{
    backlog::{BacklogRequest, BacklogResponse},
    broadcasting::{BroadcastRequest, BroadcastResponse},
    routing::{RouteAdvertisement, RouteRecall},
    state::Singleton,
    ticket::{TicketRequest, TicketResponse},
    tracing::{TraceEvent, TraceRequest, TraceResponse},
};
use crate::public::http::client::send_raw_message;

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
    BroadcastRequest(BroadcastRequest),
    BroadcastResponse(BroadcastResponse),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MessageCollection {
    pub messages: Vec<Message>,
    pub origin_id: String,
    pub destination_id: Option<String>,
}

impl TryFrom<Vec<u8>> for MessageCollection {
    type Error = anyhow::Error;

    fn try_from(bytes: Vec<u8>) -> Result<Self> {
        Ok(bincode::deserialize::<MessageCollection>(&bytes)?)
    }
}

impl TryInto<Vec<u8>> for MessageCollection {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Vec<u8>> {
        Ok(bincode::serialize::<MessageCollection>(&self)?)
    }
}

pub async fn send_message_collection_to_url(
    outgoing_message_collection: MessageCollection,
    url: String,
    singleton: Singleton,
) -> Result<MessageCollection> {
    let outgoing_message_bytes: Vec<u8> = outgoing_message_collection.try_into()?;

    let bytes = send_raw_message(outgoing_message_bytes, url, singleton.configuration).await?;

    let incoming_message_collection: MessageCollection = bytes.try_into()?;

    Ok(incoming_message_collection)
}
