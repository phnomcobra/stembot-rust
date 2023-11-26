use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::core::{
    backlog::{push_message_collection_to_backlog, request_backlog},
    broadcasting::processing::process_broadcast_response,
    message::{Message, MessageCollection},
    routing::{remove_routes_by_gateway_and_destination, RouteAdvertisement},
    state::Singleton,
    ticket::processing::{process_ticket_request, process_ticket_response},
    tracing::TraceResponse,
};

pub async fn decode_message_collection(
    singleton: Singleton,
    inbound_message_collection: &mut MessageCollection,
    outbound_message_collection: &mut MessageCollection,
) {
    let origin_id = inbound_message_collection.origin_id.clone();
    for message in &inbound_message_collection.messages {
        decode_message(
            singleton.clone(),
            message,
            &origin_id,
            outbound_message_collection,
        )
        .await;
    }
}

async fn decode_message(
    singleton: Singleton,
    message: &Message,
    origin_id: &str,
    outbound_message_collection: &mut MessageCollection,
) {
    match message {
        Message::Ping => {
            log::warn!("ping received");
            outbound_message_collection.messages.push(Message::Pong)
        }
        Message::RouteAdvertisement(advertisement) => {
            log::warn!("route advertisement received");
            advertisement
                .process(singleton.clone(), origin_id.to_owned())
                .await;
            outbound_message_collection
                .messages
                .push(Message::RouteAdvertisement(
                    RouteAdvertisement::from_routes(singleton.routes.read().await.clone()),
                ));
        }
        Message::Pong => {
            log::warn!("pong received")
        }
        Message::RouteRecall(route_recall) => {
            log::warn!("route recall received: {route_recall:?}");
            remove_routes_by_gateway_and_destination(
                origin_id.to_owned(),
                route_recall.destination_id.clone(),
                singleton.clone(),
            )
            .await
        }
        Message::BacklogRequest(backlog_request) => {
            log::warn!("backlog request received");
            let backlog_response =
                request_backlog(singleton.clone(), backlog_request.clone()).await;
            outbound_message_collection
                .messages
                .push(Message::BacklogResponse(backlog_response));
        }
        Message::BacklogResponse(backlog_response) => {
            log::warn!("backlog response received");
            for message_collection in backlog_response.message_collections.clone() {
                push_message_collection_to_backlog(message_collection.clone(), singleton.clone())
                    .await;
            }
        }
        Message::TraceRequest(trace_request) => {
            outbound_message_collection
                .messages
                .push(Message::TraceResponse(TraceResponse::from(
                    trace_request.clone(),
                )));
        }
        Message::TraceResponse(trace_response) => {
            log::info!("{trace_response}");
            let mut traces = singleton.traces.write().await;
            if let Some(trace) = traces.get_mut(&trace_response.request_id) {
                trace.stop_time = Some(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or(Duration::from_millis(0))
                        .as_millis(),
                );
            }
        }
        Message::TraceEvent(trace_event) => {
            log::info!("{trace_event}");
            let mut traces = singleton.traces.write().await;
            if let Some(trace) = traces.get_mut(&trace_event.request_id) {
                trace.events.push(trace_event.clone());
            }
        }
        Message::TicketRequest(ticket_request) => {
            log::warn!("ticket request received");
            outbound_message_collection
                .messages
                .push(Message::TicketResponse(
                    process_ticket_request(ticket_request.clone(), singleton.clone()).await,
                ));
        }
        Message::TicketResponse(ticket_response) => {
            log::warn!("ticket response received");
            process_ticket_response(ticket_response.clone(), singleton.clone()).await;
        }
        Message::BroadcastResponse(broadcast_response) => {
            process_broadcast_response(
                singleton.clone(),
                broadcast_response.clone(),
                origin_id.into(),
            )
            .await
        }
        _ => {}
    }
}
