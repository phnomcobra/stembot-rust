pub mod exchange;
pub mod processing;

use serde::{Deserialize, Serialize};
use std::fmt::Display;

use crate::core::{peering::PeerQuery, routing::RouteQuery, tracing::Trace};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Ticket {
    Test,
    BeginTrace(Trace),
    DrainTrace(Trace),
    PollTrace(Trace),
    RouteQuery(RouteQuery),
    PeerQuery(PeerQuery),
    TicketQuery(TicketQuery),
}

impl Display for Ticket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Ticket::Test => write!(f, "Test"),
            Ticket::BeginTrace(_) => write!(f, "BeginTrace"),
            Ticket::DrainTrace(_) => write!(f, "DrainTrace"),
            Ticket::PollTrace(_) => write!(f, "PollTrace"),
            Ticket::RouteQuery(_) => write!(f, "RouteQuery"),
            Ticket::PeerQuery(_) => write!(f, "PeerQuery"),
            Ticket::TicketQuery(_) => write!(f, "TicketQuery"),
        }
    }
}

pub type TicketSlice = (TicketRequest, Option<Ticket>);

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TicketQuery {
    pub tickets: Option<Vec<(String, TicketSlice)>>,
}

impl Display for TicketRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {:?} {}",
            self.ticket_id, self.origin_id, self.destination_id, self.ticket
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketRequest {
    pub ticket: Ticket,
    pub ticket_id: String,
    pub start_time: u128,
    pub destination_id: Option<String>,
    pub origin_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketResponse {
    pub ticket: Ticket,
    pub ticket_id: String,
    pub start_time: u128,
}
