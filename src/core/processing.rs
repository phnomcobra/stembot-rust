use crate::core::{
    backlog::{push_message_collection_to_backlog, request_backlog},
    messaging::{
        send_message_collection_to_url, Message, MessageCollection, RouteAdvertisement,
        RouteRecall, TraceResponse,
    },
    peering::{lookup_peer_url, touch_peer},
    routing::{remove_routes_by_gateway_and_destination, remove_routes_by_url, resolve_gateway_id},
    state::Singleton,
    ticketing::{process_ticket_request, process_ticket_response},
};

pub async fn process_message_collection(
    mut inbound_message_collection: MessageCollection,
    singleton: Singleton,
) -> MessageCollection {
    let mut outbound_message_collection = MessageCollection {
        messages: vec![],
        origin_id: singleton.configuration.id.clone(),
        destination_id: Some(inbound_message_collection.origin_id.clone()),
    };

    // If the destination id was not known at the time of sending,
    // then it is implied that the message collection was intended to be processed
    // at this id.
    let destination_id = match inbound_message_collection.destination_id.clone() {
        Some(id) => id,
        None => singleton.configuration.id.clone(),
    };

    // Process trace messages
    for message in inbound_message_collection.messages.iter_mut() {
        match message {
            Message::TraceRequest(trace_request) => {
                let trace_event = trace_request.process(singleton.clone());

                let message_collection = MessageCollection {
                    messages: vec![Message::TraceEvent(trace_event)],
                    destination_id: Some(inbound_message_collection.origin_id.clone()),
                    origin_id: singleton.configuration.id.clone(),
                };

                push_message_collection_to_backlog(message_collection, singleton.clone()).await
            }
            Message::TraceResponse(trace_response) => {
                let trace_event = trace_response.process(singleton.clone());

                if inbound_message_collection.destination_id.is_some() {
                    let message_collection = MessageCollection {
                        messages: vec![Message::TraceEvent(trace_event)],
                        destination_id: inbound_message_collection.destination_id.clone(),
                        origin_id: singleton.configuration.id.clone(),
                    };

                    push_message_collection_to_backlog(message_collection, singleton.clone()).await
                }
            }
            _ => {}
        }
    }

    let gateway_id = resolve_gateway_id(destination_id.clone(), singleton.clone()).await;

    if gateway_id == Some(singleton.configuration.id.clone()) {
        touch_peer(&inbound_message_collection.origin_id, singleton.clone()).await;

        for message in inbound_message_collection.messages {
            match message {
                Message::Ping => {
                    log::warn!("ping received");
                    outbound_message_collection.messages.push(Message::Pong)
                }
                Message::RouteAdvertisement(advertisement) => {
                    log::warn!("route advertisement received");
                    advertisement
                        .process(
                            singleton.clone(),
                            inbound_message_collection.origin_id.clone(),
                        )
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
                        inbound_message_collection.origin_id.clone(),
                        route_recall.destination_id,
                        singleton.clone(),
                    )
                    .await
                }
                Message::BacklogRequest(backlog_request) => {
                    log::warn!("backlog request received");
                    let backlog_response =
                        request_backlog(singleton.clone(), backlog_request).await;
                    outbound_message_collection
                        .messages
                        .push(Message::BacklogResponse(backlog_response));
                }
                Message::BacklogResponse(backlog_response) => {
                    log::warn!("backlog response received");
                    for message_collection in backlog_response.message_collections {
                        push_message_collection_to_backlog(
                            message_collection.clone(),
                            singleton.clone(),
                        )
                        .await;
                    }
                }
                Message::TraceRequest(trace_request) => {
                    outbound_message_collection
                        .messages
                        .push(Message::TraceResponse(TraceResponse::from(trace_request)));
                }
                Message::TraceResponse(trace_response) => {
                    log::info!("{trace_response}")
                }
                Message::TraceEvent(trace_event) => {
                    log::info!("{trace_event}");
                    let mut traces = singleton.traces.write().await;
                    match traces.get_mut(&trace_event.request_id) {
                        Some(events) => {
                            events.push(trace_event);
                        }
                        None => {
                            traces.insert(trace_event.request_id.clone(), vec![trace_event]);
                        }
                    };
                }
                Message::TicketRequest(ticket_request) => {
                    log::warn!("ticket request received");
                    outbound_message_collection
                        .messages
                        .push(Message::TicketResponse(
                            process_ticket_request(ticket_request, singleton.clone()).await,
                        ));
                }
                Message::TicketResponse(ticket_response) => {
                    log::warn!("ticket response received");
                    process_ticket_response(ticket_response, singleton.clone()).await;
                }
            }
        }
    } else {
        // Forwarding stuff happens here
        match gateway_id {
            Some(gateway_id) => {
                let url = lookup_peer_url(&gateway_id, singleton.clone()).await;
                match url {
                    // Forward the message collection
                    Some(url) => {
                        match send_message_collection_to_url(
                            inbound_message_collection.clone(),
                            url.clone(),
                            singleton.clone(),
                        )
                        .await
                        {
                            Ok(message) => {
                                push_message_collection_to_backlog(message, singleton.clone()).await
                            }
                            Err(_) => {
                                // Encountered dead url
                                // Remove all routes using that url and push message into backlog
                                remove_routes_by_url(url.clone(), singleton.clone()).await;

                                push_message_collection_to_backlog(
                                    inbound_message_collection,
                                    singleton.clone(),
                                )
                                .await;
                            }
                        }
                    }
                    // Know where to forward to but not how
                    // Message will have to be pulled
                    None => {
                        push_message_collection_to_backlog(
                            inbound_message_collection,
                            singleton.clone(),
                        )
                        .await
                    }
                }
            }
            // Don't know where to forward to
            None => {
                let destination_ids: Vec<String> = singleton
                    .peers
                    .read()
                    .await
                    .iter()
                    .filter(|x| x.id.is_some())
                    .filter(|x| x.id != inbound_message_collection.destination_id)
                    .map(|x| x.id.clone().unwrap())
                    .collect();

                if inbound_message_collection.destination_id.is_some() {
                    for id in destination_ids.iter() {
                        push_message_collection_to_backlog(
                            MessageCollection {
                                messages: vec![Message::RouteRecall(RouteRecall {
                                    destination_id: inbound_message_collection
                                        .destination_id
                                        .clone()
                                        .unwrap(),
                                })],
                                origin_id: singleton.configuration.id.clone(),
                                destination_id: Some(id.clone()),
                            },
                            singleton.clone(),
                        )
                        .await;
                    }
                }

                push_message_collection_to_backlog(inbound_message_collection, singleton.clone())
                    .await;
            }
        }
    }

    outbound_message_collection
}
