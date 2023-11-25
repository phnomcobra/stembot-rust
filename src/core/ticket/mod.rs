pub mod exchange;
pub mod processing;

use serde::{Deserialize, Serialize};
use std::fmt::Display;

use crate::core::{
    broadcasting::Broadcast, peering::PeerQuery, routing::RouteQuery, tracing::Trace,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TicketMessage {
    Test,
    BeginTrace(Trace),
    DrainTrace(Trace),
    PollTrace(Trace),
    RouteQuery(RouteQuery),
    PeerQuery(PeerQuery),
    TicketQuery(TicketQuery),
    BeginBroadcast(Broadcast),
    DrainBroadcast(Broadcast),
    PollBroadcast(Broadcast),
}

impl Display for TicketMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            TicketMessage::Test => write!(f, "Test"),
            TicketMessage::BeginTrace(_) => write!(f, "BeginTrace"),
            TicketMessage::DrainTrace(_) => write!(f, "DrainTrace"),
            TicketMessage::PollTrace(_) => write!(f, "PollTrace"),
            TicketMessage::RouteQuery(_) => write!(f, "RouteQuery"),
            TicketMessage::PeerQuery(_) => write!(f, "PeerQuery"),
            TicketMessage::TicketQuery(_) => write!(f, "TicketQuery"),
            TicketMessage::BeginBroadcast(_) => write!(f, "BeginBroadcast"),
            TicketMessage::DrainBroadcast(_) => write!(f, "DrainBroadcast"),
            TicketMessage::PollBroadcast(_) => write!(f, "PollBroadcast"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ticket {
    request: TicketRequest,
    response: Option<TicketMessage>,
    destination_id: Option<String>,
    start_time: u128,
    stop_time: Option<u128>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TicketQuery {
    pub tickets: Option<Vec<Ticket>>,
}

impl Display for Ticket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let progress = if self.response.is_some() {
            "(complete)"
        } else {
            "(in progress)"
        };

        write!(
            f,
            "{} {:?} {} {}",
            self.request.ticket_id, self.destination_id, self.request.ticket_message, progress
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketRequest {
    pub ticket_message: TicketMessage,
    pub ticket_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketResponse {
    pub ticket_message: TicketMessage,
    pub ticket_id: String,
}
