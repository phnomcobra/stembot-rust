use crate::core::{
    backlog::{push_message_collection_to_backlog, request_backlog},
    messaging::{Message, MessageCollection},
    routing::{remove_routes_by_gateway_and_destination, RouteAdvertisement},
    state::Singleton,
    ticketing::processing::{process_ticket_request, process_ticket_response},
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
            log::warn!("route recall received");
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
            log::info!("{trace_response}")
        }
        Message::TraceEvent(trace_event) => {
            log::info!("{trace_event}");
            let mut traces = singleton.traces.write().await;
            match traces.get_mut(&trace_event.request_id) {
                Some(events) => {
                    events.push(trace_event.clone());
                }
                None => {
                    traces.insert(trace_event.request_id.clone(), vec![trace_event.clone()]);
                }
            };
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
    }
}
