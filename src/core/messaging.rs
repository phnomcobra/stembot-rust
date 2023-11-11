use crate::core::{routing::Route, state::Singleton};
use crate::public::http::client::send_raw_message;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
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
pub struct Trace {
    pub events: Vec<TraceEvent>,
    pub period: Option<u64>,
    pub request_id: Option<String>,
    pub destination_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Ticket {
    Test,
    BeginTrace(Trace),
    DrainTrace(Trace),
    SyncTrace(Trace),
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
