use std::{
    error::Error,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::sleep;

use crate::{
    backlog::push_message_collection_to_backlog,
    messaging::{Message, MessageCollection, Ticket, TicketRequest, TicketResponse},
    state::Singleton,
};

pub async fn process_ticket_request(ticket_request: TicketRequest) -> TicketResponse {
    match ticket_request.ticket {
        Ticket::Test => TicketResponse {
            ticket: ticket_request.ticket,
            ticket_id: ticket_request.ticket_id,
            start_time: ticket_request.start_time,
        },
    }
}

pub async fn process_ticket_response(ticket_response: TicketResponse, singleton: Singleton) {
    let mut ticket_map = singleton.ticket_map.write().await;
    ticket_map.insert(ticket_response.ticket_id, Some(ticket_response.ticket));
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
        .ticket_map
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

pub async fn receive_ticket(
    ticket_id: String,
    singleton: Singleton,
) -> Result<Ticket, Box<dyn Error + Send + Sync + 'static>> {
    let mut millis_to_delay: u64 = 10;
    let mut ticket: Option<Ticket> = None;

    while millis_to_delay < singleton.configuration.ticketexpiration {
        let mut ticket_map = singleton.ticket_map.write().await;
        match ticket_map.get(&ticket_id) {
            Some(ticket_option) => match ticket_option {
                Some(ticket_value) => {
                    ticket = Some(ticket_value.clone());
                    ticket_map.remove(&ticket_id);
                    drop(ticket_map);
                    break;
                }
                None => {
                    drop(ticket_map);
                    sleep(Duration::from_millis(millis_to_delay)).await;
                    millis_to_delay *= 2;
                }
            },
            None => {
                drop(ticket_map);
                break;
            }
        }
    }

    match ticket {
        Some(ticket) => Ok(ticket),
        None => Err(format!("timeout exceeded for ticket {ticket_id}").into()),
    }
}

pub async fn synchronize_ticket(
    ticket: Ticket,
    ticket_id: Option<String>,
    destination_id: Option<String>,
    singleton: Singleton,
) -> Result<Ticket, Box<dyn Error + Send + Sync + 'static>> {
    let ticket_id = send_ticket(ticket, ticket_id, destination_id, singleton.clone()).await;
    receive_ticket(ticket_id, singleton.clone()).await
}
