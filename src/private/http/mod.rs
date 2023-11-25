use serde::{Deserialize, Serialize};

use crate::core::ticket::TicketMessage;

pub mod client;
pub mod server;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Session {
    ticket_message: Option<TicketMessage>,
    ticket_id: Option<String>,
    destination_id: Option<String>,
}
