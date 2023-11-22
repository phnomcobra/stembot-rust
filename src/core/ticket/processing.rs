use crate::core::{
    backlog::push_message_collection_to_backlog,
    message::{Message, MessageCollection},
    state::Singleton,
    tracing::TraceRequest,
};

use super::{Ticket, TicketRequest, TicketResponse};

pub async fn process_ticket_request(
    ticket_request: TicketRequest,
    singleton: Singleton,
) -> TicketResponse {
    match ticket_request.ticket {
        Ticket::TicketQuery(query) => {
            let mut query = query.clone();

            let mut results = vec![];
            let tickets = singleton.tickets.read().await;

            for (ticket_id, ticket) in tickets.iter() {
                results.push((ticket_id.clone(), ticket.clone()));
            }

            query.tickets = Some(results);

            TicketResponse {
                ticket: Ticket::TicketQuery(query),
                ticket_id: ticket_request.ticket_id,
                start_time: ticket_request.start_time,
            }
        }
        Ticket::PeerQuery(query) => {
            let mut query = query.clone();

            let peers = singleton.peers.read().await;

            query.peers = Some(peers.clone());

            TicketResponse {
                ticket: Ticket::PeerQuery(query),
                ticket_id: ticket_request.ticket_id,
                start_time: ticket_request.start_time,
            }
        }
        Ticket::RouteQuery(query) => {
            let mut query = query.clone();

            let routes = singleton.routes.read().await;

            match (&query.destination_ids, &query.gateway_ids) {
                (Some(destination_ids), Some(gateway_ids)) => {
                    query.routes = Some(
                        routes
                            .iter()
                            .filter(|route| destination_ids.contains(&route.destination_id))
                            .filter(|route| gateway_ids.contains(&route.gateway_id))
                            .cloned()
                            .collect(),
                    );
                }
                (Some(destination_ids), None) => {
                    query.routes = Some(
                        routes
                            .iter()
                            .filter(|route| destination_ids.contains(&route.destination_id))
                            .cloned()
                            .collect(),
                    );
                }
                (None, Some(gateway_ids)) => {
                    query.routes = Some(
                        routes
                            .iter()
                            .filter(|route| gateway_ids.contains(&route.gateway_id))
                            .cloned()
                            .collect(),
                    );
                }
                (None, None) => {
                    query.routes = Some(routes.clone());
                }
            }

            TicketResponse {
                ticket: Ticket::RouteQuery(query),
                ticket_id: ticket_request.ticket_id,
                start_time: ticket_request.start_time,
            }
        }
        Ticket::Test => TicketResponse {
            ticket: ticket_request.ticket,
            ticket_id: ticket_request.ticket_id,
            start_time: ticket_request.start_time,
        },
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
    match tickets.get_mut(&ticket_response.ticket_id) {
        Some(ticket) => ticket.1 = Some(ticket_response.ticket),
        None => log::warn!(
            "unsolicited ticket response received!: {}",
            ticket_response.ticket_id
        ),
    }
}
