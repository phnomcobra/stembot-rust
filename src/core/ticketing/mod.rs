pub mod processing;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

use crate::core::{
    backlog::push_message_collection_to_backlog,
    messaging::{Message, MessageCollection},
    state::Singleton,
    tracing::Trace,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Ticket {
    Test,
    BeginTrace(Trace),
    DrainTrace(Trace),
    PollTrace(Trace),
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

pub async fn send_ticket(
    ticket: Ticket,
    ticket_id: Option<String>,
    destination_id: Option<String>,
    singleton: Singleton,
) -> String {
    let ticket_id = match ticket_id {
        Some(id) => id,
        None => rand::random::<usize>().to_string(),
    };

    singleton
        .tickets
        .write()
        .await
        .insert(ticket_id.clone(), None);

    let start_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_millis(0))
        .as_millis();

    let ticket_request = TicketRequest {
        ticket_id: ticket_id.clone(),
        ticket,
        start_time,
    };

    let message_collection = MessageCollection {
        origin_id: singleton.configuration.id.clone(),
        destination_id,
        messages: vec![Message::TicketRequest(ticket_request)],
    };

    push_message_collection_to_backlog(message_collection, singleton.clone()).await;

    ticket_id
}

pub async fn receive_ticket(ticket_id: String, singleton: Singleton) -> Result<Ticket> {
    let mut millis_to_delay: u64 = 10;
    let mut ticket: Option<Ticket> = None;

    while millis_to_delay < singleton.configuration.ticketexpiration {
        let mut tickets = singleton.tickets.write().await;
        match tickets.get(&ticket_id) {
            Some(ticket_option) => match ticket_option {
                Some(ticket_value) => {
                    ticket = Some(ticket_value.clone());
                    tickets.remove(&ticket_id);
                    drop(tickets);
                    break;
                }
                None => {
                    drop(tickets);
                    sleep(Duration::from_millis(millis_to_delay)).await;
                    millis_to_delay *= 2;
                }
            },
            None => {
                drop(tickets);
                break;
            }
        }
    }

    match ticket {
        Some(ticket) => Ok(ticket),
        None => Err(anyhow::Error::msg(format!(
            "timeout exceeded for ticket {ticket_id}"
        ))),
    }
}

pub async fn send_and_receive_ticket(
    ticket: Ticket,
    ticket_id: Option<String>,
    destination_id: Option<String>,
    singleton: Singleton,
) -> Result<Ticket> {
    let ticket_id = send_ticket(ticket, ticket_id, destination_id, singleton.clone()).await;
    receive_ticket(ticket_id, singleton.clone()).await
}
