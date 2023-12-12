use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::core::{
    backlog::push_message_collection_to_backlog,
    message::{Message, MessageCollection},
    state::Singleton,
    tracing::TraceRequest,
};

use super::{TicketMessage, TicketRequest, TicketResponse};

pub async fn process_ticket_request(
    ticket_request: TicketRequest,
    singleton: Singleton,
) -> TicketResponse {
    match ticket_request.ticket_message {
        TicketMessage::TicketQuery(query) => {
            let mut query = query.clone();

            let mut results = vec![];
            let tickets = singleton.tickets.read().await;

            for ticket_state in tickets.values() {
                results.push(ticket_state.clone());
            }

            query.tickets = Some(results);

            TicketResponse {
                ticket_message: TicketMessage::TicketQuery(query),
                ticket_id: ticket_request.ticket_id,
            }
        }
        TicketMessage::PeerQuery(query) => {
            let mut query = query.clone();

            let peers = singleton.peers.read().await;

            query.peers = Some(peers.clone());

            TicketResponse {
                ticket_message: TicketMessage::PeerQuery(query),
                ticket_id: ticket_request.ticket_id,
            }
        }
        TicketMessage::RouteQuery(query) => {
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
                ticket_message: TicketMessage::RouteQuery(query),
                ticket_id: ticket_request.ticket_id,
            }
        }
        TicketMessage::Test => TicketResponse {
            ticket_message: ticket_request.ticket_message,
            ticket_id: ticket_request.ticket_id,
        },
        TicketMessage::BeginTrace(trace) => {
            let mut trace = trace.clone();

            trace.start_time = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::from_millis(0))
                    .as_millis(),
            );

            let trace_request = match trace.request_id.clone() {
                Some(request_id) => TraceRequest::new(request_id),
                None => TraceRequest::default(),
            };

            let mut traces = singleton.traces.write().await;

            let request_id = trace_request.request_id.clone();
            trace.request_id = Some(request_id.clone());
            if traces.contains_key(&request_id) {
                traces.remove(&request_id);
            }

            traces.insert(request_id.clone(), trace.clone());

            let trace_request_message = Message::TraceRequest(trace_request);

            let message_collection = MessageCollection {
                origin_id: singleton.configuration.id.clone(),
                destination_id: Some(trace.destination_id.clone()),
                messages: vec![trace_request_message.clone()],
            };

            push_message_collection_to_backlog(message_collection, singleton.clone()).await;

            TicketResponse {
                ticket_message: TicketMessage::BeginTrace(trace),
                ticket_id: ticket_request.ticket_id,
            }
        }
        TicketMessage::DrainTrace(trace) => {
            let mut trace = trace.clone();

            let mut traces = singleton.traces.write().await;

            if let Some(request_id) = trace.request_id.clone() {
                if let Some(updated_trace) = traces.get(&request_id) {
                    trace = updated_trace.clone();
                    traces.remove(&request_id);
                }
            }

            TicketResponse {
                ticket_message: TicketMessage::DrainTrace(trace),
                ticket_id: ticket_request.ticket_id,
            }
        }
        TicketMessage::PollTrace(trace) => {
            let mut trace = trace.clone();

            if let Some(request_id) = trace.request_id.clone() {
                if let Some(updated_trace) = singleton.traces.read().await.get(&request_id) {
                    trace = updated_trace.clone();
                }
            }

            TicketResponse {
                ticket_message: TicketMessage::PollTrace(trace),
                ticket_id: ticket_request.ticket_id,
            }
        }
        TicketMessage::BeginBroadcast(broadcast) => {
            let mut broadcast = broadcast.clone();

            if broadcast.request.origin_id.is_some() {
                broadcast.request.origin_id = Some(singleton.configuration.id.clone());

                singleton
                    .broadcasts
                    .write()
                    .await
                    .insert(broadcast.request.request_id.clone(), broadcast.clone());
            }

            let message_collection = MessageCollection {
                origin_id: singleton.configuration.id.clone(),
                destination_id: None,
                messages: vec![Message::BroadcastRequest(broadcast.request.clone())],
            };

            push_message_collection_to_backlog(message_collection, singleton.clone()).await;

            TicketResponse {
                ticket_message: TicketMessage::BeginBroadcast(broadcast),
                ticket_id: ticket_request.ticket_id,
            }
        }
        TicketMessage::PollBroadcast(broadcast) => {
            let mut broadcast = broadcast.clone();

            let broadcasts = singleton.broadcasts.read().await;

            if let Some(stored_broadcast) = broadcasts.get(&broadcast.request.request_id) {
                broadcast.responses = stored_broadcast.responses.clone()
            }

            TicketResponse {
                ticket_message: TicketMessage::PollBroadcast(broadcast),
                ticket_id: ticket_request.ticket_id,
            }
        }
        TicketMessage::DrainBroadcast(broadcast) => {
            let mut broadcast = broadcast.clone();

            let mut broadcasts = singleton.broadcasts.write().await;

            if let Some(stored_broadcast) = broadcasts.get(&broadcast.request.request_id) {
                broadcast.responses = stored_broadcast.responses.clone();
                broadcasts.remove(&broadcast.request.request_id);
            }

            TicketResponse {
                ticket_message: TicketMessage::DrainBroadcast(broadcast),
                ticket_id: ticket_request.ticket_id,
            }
        }
    }
}

pub async fn process_ticket_response(ticket_response: TicketResponse, singleton: Singleton) {
    let mut tickets = singleton.tickets.write().await;
    match tickets.get_mut(&ticket_response.ticket_id) {
        Some(ticket_state) => {
            ticket_state.response = Some(ticket_response.ticket_message);

            ticket_state.stop_time = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::from_millis(0))
                    .as_millis(),
            )
        }
        None => log::warn!(
            "unsolicited ticket response received!: {}",
            ticket_response.ticket_id
        ),
    }
}
