use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

use crate::core::{
    backlog::push_message_collection_to_backlog,
    messaging::{Message, MessageCollection},
    state::Singleton,
};

use anyhow::Result;

use super::tracing::{Trace, TraceRequest};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Ticket {
    Test,
    BeginTrace(Trace),
    DrainTrace(Trace),
    SyncTrace(Trace),
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
        Ticket::SyncTrace(trace) => {
            let mut trace = trace.clone();

            let trace_request = match trace.request_id.clone() {
                Some(request_id) => TraceRequest::new(request_id),
                None => TraceRequest::default(),
            };

            let request_id = trace_request.request_id.clone();

            trace.request_id = Some(request_id.clone());

            let trace_request_message = Message::TraceRequest(trace_request);

            let message_collection = MessageCollection {
                origin_id: singleton.configuration.id.clone(),
                destination_id: Some(trace.destination_id.clone()),
                messages: vec![trace_request_message.clone()],
            };

            push_message_collection_to_backlog(message_collection, singleton.clone()).await;

            sleep(Duration::from_millis(trace.period.unwrap_or(0))).await;

            let mut traces = singleton.traces.write().await;

            trace.events = traces.get(&request_id).unwrap().to_vec();
            traces.remove(&request_id);

            TicketResponse {
                ticket: Ticket::SyncTrace(trace),
                ticket_id: ticket_request.ticket_id,
                start_time: ticket_request.start_time,
            }
        }
        Ticket::BeginTrace(trace) => {
            let mut trace = trace.clone();

            let trace_request = match trace.request_id.clone() {
                Some(request_id) => TraceRequest::new(request_id),
                None => TraceRequest::default(),
            };

            let request_id = trace_request.request_id.clone();

            trace.request_id = Some(request_id.clone());

            let trace_request_message = Message::TraceRequest(trace_request);

            let message_collection = MessageCollection {
                origin_id: singleton.configuration.id.clone(),
                destination_id: Some(trace.destination_id.clone()),
                messages: vec![trace_request_message.clone()],
            };

            push_message_collection_to_backlog(message_collection, singleton.clone()).await;

            TicketResponse {
                ticket: Ticket::BeginTrace(trace),
                ticket_id: ticket_request.ticket_id,
                start_time: ticket_request.start_time,
            }
        }
        Ticket::DrainTrace(trace) => {
            let mut trace = trace.clone();

            let mut traces = singleton.traces.write().await;

            if trace.request_id.is_some() {
                let request_id = trace.request_id.clone().unwrap();
                trace.events = traces.get(&request_id).unwrap().to_vec();
                traces.remove(&request_id);
            }

            TicketResponse {
                ticket: Ticket::DrainTrace(trace),
                ticket_id: ticket_request.ticket_id,
                start_time: ticket_request.start_time,
            }
        }
        Ticket::PollTrace(trace) => {
            let mut trace = trace.clone();

            let traces = singleton.traces.read().await;

            if trace.request_id.is_some() {
                let request_id = trace.request_id.clone().unwrap();
                trace.events = traces.get(&request_id).unwrap().to_vec();
            }

            TicketResponse {
                ticket: Ticket::PollTrace(trace),
                ticket_id: ticket_request.ticket_id,
                start_time: ticket_request.start_time,
            }
        }
    }
}

pub async fn process_ticket_response(ticket_response: TicketResponse, singleton: Singleton) {
    let mut tickets = singleton.tickets.write().await;
    tickets.insert(ticket_response.ticket_id, Some(ticket_response.ticket));
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
