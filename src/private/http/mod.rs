use serde::{Deserialize, Serialize};

use crate::core::ticketing::Ticket;

pub mod client;
pub mod server;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Session {
    ticket: Option<Ticket>,
    ticket_id: Option<String>,
    destination_id: Option<String>,
}
