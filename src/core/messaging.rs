use crate::core::state::Singleton;
use crate::public::http::client::send_raw_message;
use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{
    backlog::{BacklogRequest, BacklogResponse},
    routing::{RouteAdvertisement, RouteRecall},
    ticketing::{TicketRequest, TicketResponse},
    tracing::{TraceEvent, TraceRequest, TraceResponse},
};

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
