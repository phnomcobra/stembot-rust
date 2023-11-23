use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use tokio::time::sleep;

use crate::core::{
    backlog::push_message_collection_to_backlog,
    message::{Message, MessageCollection},
    state::Singleton,
};

use super::{Ticket, TicketRequest, TicketState};

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

    let start_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_millis(0))
        .as_millis();

    let ticket_request = TicketRequest {
        ticket_id: ticket_id.clone(),
        ticket,
    };

    let ticket_state = TicketState {
        request: ticket_request.clone(),
        response: None,
        destination_id: destination_id.clone(),
        start_time,
        stop_time: None,
    };

    singleton
        .tickets
        .write()
        .await
        .insert(ticket_id.clone(), ticket_state);

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
            Some(ticket_state) => match ticket_state.response.clone() {
                Some(ticket_response) => {
                    ticket = Some(ticket_response.clone());
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
