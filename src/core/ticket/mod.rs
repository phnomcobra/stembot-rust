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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketState {
    request: TicketRequest,
    response: Option<Ticket>,
    destination_id: Option<String>,
    start_time: u128,
    stop_time: Option<u128>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TicketQuery {
    pub tickets: Option<Vec<TicketState>>,
}

impl Display for TicketState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let progress = if self.response.is_some() {
            "(complete)"
        } else {
            "(in progress)"
        };

        write!(
            f,
            "{} {:?} {} {}",
            self.request.ticket_id, self.destination_id, self.request.ticket, progress
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketRequest {
    pub ticket: Ticket,
    pub ticket_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketResponse {
    pub ticket: Ticket,
    pub ticket_id: String,
}
