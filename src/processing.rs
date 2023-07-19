use std::sync::Arc;

use tokio::sync::RwLock;

use crate::{
    backlog::{push_message_collection_to_backlog, request_backlog},
    config::Configuration,
    messaging::{
        send_message_collection_to_url, Message, MessageCollection, RouteAdvertisement, RouteRecall,
    },
    peering::{lookup_peer_url, touch_peer, Peer},
    routing::{
        remove_routes_by_gateway_and_destination, remove_routes_by_url, resolve_gateway_id, Route,
    },
};

pub async fn process_message_collection<T: Into<MessageCollection>, U: Into<Configuration>>(
    inbound_message_collection: T,
    configuration: U,
    peering_table: Arc<RwLock<Vec<Peer>>>,
    routing_table: Arc<RwLock<Vec<Route>>>,
    message_backlog: Arc<RwLock<Vec<MessageCollection>>>,
) -> MessageCollection {
    let configuration = configuration.into();

    let inbound_message_collection = inbound_message_collection.into();

    let mut outbound_message_collection = MessageCollection {
        messages: vec![],
        origin_id: configuration.id.clone(),
        destination_id: Some(inbound_message_collection.origin_id.clone()),
    };

    // If the destination id was not known at the time of sending,
    // then it is implied that the message collection was intended to be processed
    // at this id.
    let destination_id = match inbound_message_collection.destination_id.clone() {
        Some(id) => id,
        None => configuration.id.clone(),
    };

    let gateway_id = resolve_gateway_id(destination_id.clone(), routing_table.clone()).await;

    if gateway_id == Some(configuration.id.clone()) {
        touch_peer(&inbound_message_collection.origin_id, peering_table.clone()).await;

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
                            configuration.clone(),
                            routing_table.clone(),
                            inbound_message_collection.origin_id.clone(),
                        )
                        .await;
                    outbound_message_collection
                        .messages
                        .push(Message::RouteAdvertisement(
                            RouteAdvertisement::from_routes(routing_table.read().await.clone()),
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
                        routing_table.clone(),
                    )
                    .await
                }
                Message::BacklogRequest(backlog_request) => {
                    log::warn!("backlog request received");
                    let backlog_response = request_backlog(
                        configuration.clone(),
                        routing_table.clone(),
                        message_backlog.clone(),
                        backlog_request.clone(),
                    )
                    .await;
                    outbound_message_collection
                        .messages
                        .push(Message::BacklogResponse(backlog_response));
                }
                Message::BacklogResponse(backlog_response) => {
                    log::warn!("backlog response received");
                    for message_collection in backlog_response.message_collections {
                        push_message_collection_to_backlog(
                            message_collection.clone(),
                            message_backlog.clone(),
                        )
                        .await;
                    }
                }
            }
        }
    } else {
        // Forwarding stuff happens here
        match gateway_id {
            Some(gateway_id) => {
                let url = lookup_peer_url(&gateway_id, peering_table.clone()).await;
                match url {
                    // Forward the message collection
                    Some(url) => {
                        match send_message_collection_to_url(
                            inbound_message_collection.clone(),
                            url.clone(),
                            configuration.clone(),
                        )
                        .await
                        {
                            Ok(message) => {
                                push_message_collection_to_backlog(message, message_backlog.clone())
                                    .await
                            }
                            Err(_) => {
                                // Encountered dead url
                                // Remove all routes using that url and push message into backlog
                                remove_routes_by_url(
                                    url.clone(),
                                    routing_table.clone(),
                                    peering_table.clone(),
                                )
                                .await;

                                push_message_collection_to_backlog(
                                    inbound_message_collection,
                                    message_backlog.clone(),
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
                            message_backlog.clone(),
                        )
                        .await
                    }
                }
            }
            // Don't know where to forward to
            None => {
                let destination_ids: Vec<String> = peering_table
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
                                origin_id: configuration.id.clone(),
                                destination_id: Some(id.clone()),
                            },
                            message_backlog.clone(),
                        )
                        .await;
                    }
                }

                push_message_collection_to_backlog(
                    inbound_message_collection,
                    message_backlog.clone(),
                )
                .await;
            }
        }
    }

    outbound_message_collection
}
