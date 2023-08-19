use std::{
    error::Error,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::sleep;

use crate::core::{
    backlog::push_message_collection_to_backlog,
    messaging::{Message, MessageCollection, Ticket, TicketRequest, TicketResponse},
    state::Singleton,
};

use super::messaging::TraceRequest;

pub async fn process_ticket_request(
    ticket_request: TicketRequest,
    singleton: Singleton,
) -> TicketResponse {
    match ticket_request.ticket {
        Ticket::Test => TicketResponse {
            ticket: ticket_request.ticket,
            ticket_id: ticket_request.ticket_id,
            start_time: ticket_request.start_time,
        },
        Ticket::SyncTrace(ticket) => {
            let mut ticket = ticket.clone();

            let trace_request = match ticket.request_id.clone() {
                Some(request_id) => TraceRequest::new(request_id),
                None => TraceRequest::default(),
            };

            let request_id = trace_request.request_id.clone();

            ticket.request_id = Some(request_id.clone());

            let trace_request_message = Message::TraceRequest(trace_request);

            let message_collection = MessageCollection {
                origin_id: singleton.configuration.id.clone(),
                destination_id: Some(ticket.destination_id.clone()),
                messages: vec![trace_request_message.clone()],
            };

            push_message_collection_to_backlog(message_collection, singleton.clone()).await;

            sleep(Duration::from_millis(ticket.period.unwrap_or(0))).await;

            let mut trace_map = singleton.trace_map.write().await;

            ticket.events = trace_map.get(&request_id).unwrap().to_vec();
            trace_map.remove(&request_id);

            TicketResponse {
                ticket: Ticket::SyncTrace(ticket),
                ticket_id: ticket_request.ticket_id,
                start_time: ticket_request.start_time,
            }
        }
        Ticket::BeginTrace(ticket) => {
            let mut ticket = ticket.clone();

            let trace_request = match ticket.request_id.clone() {
                Some(request_id) => TraceRequest::new(request_id),
                None => TraceRequest::default(),
            };

            let request_id = trace_request.request_id.clone();

            ticket.request_id = Some(request_id.clone());

            let trace_request_message = Message::TraceRequest(trace_request);

            let message_collection = MessageCollection {
                origin_id: singleton.configuration.id.clone(),
                destination_id: Some(ticket.destination_id.clone()),
                messages: vec![trace_request_message.clone()],
            };

            push_message_collection_to_backlog(message_collection, singleton.clone()).await;

            TicketResponse {
                ticket: Ticket::BeginTrace(ticket),
                ticket_id: ticket_request.ticket_id,
                start_time: ticket_request.start_time,
            }
        }
        Ticket::DrainTrace(ticket) => {
            let mut ticket = ticket.clone();

            let mut trace_map = singleton.trace_map.write().await;

            if ticket.request_id.is_some() {
                let request_id = ticket.request_id.clone().unwrap();
                ticket.events = trace_map.get(&request_id).unwrap().to_vec();
                trace_map.remove(&request_id);
            }

            TicketResponse {
                ticket: Ticket::DrainTrace(ticket),
                ticket_id: ticket_request.ticket_id,
                start_time: ticket_request.start_time,
            }
        }
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

pub async fn send_and_receive_ticket(
    ticket: Ticket,
    ticket_id: Option<String>,
    destination_id: Option<String>,
    singleton: Singleton,
) -> Result<Ticket, Box<dyn Error + Send + Sync + 'static>> {
    let ticket_id = send_ticket(ticket, ticket_id, destination_id, singleton.clone()).await;
    receive_ticket(ticket_id, singleton.clone()).await
}
