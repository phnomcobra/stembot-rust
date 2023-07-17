use std::sync::{Arc, RwLock};

use crate::{
    config::Configuration,
    messaging::{send_message_collection_to_url, Message, MessageCollection, RouteRecall},
    peering::{check_peer, lookup_peer_url, Peer},
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

    check_peer(&inbound_message_collection.origin_id, peering_table.clone());

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

    let gateway_id = resolve_gateway_id(destination_id.clone(), routing_table.clone());

    if gateway_id == Some(configuration.id.clone()) {
        for message in inbound_message_collection.messages {
            match message {
                Message::Ping => outbound_message_collection.messages.push(Message::Pong),
                Message::RouteAdvertisement(advertisement) => advertisement.process(
                    configuration.clone(),
                    routing_table.clone(),
                    inbound_message_collection.origin_id.clone(),
                ),
                Message::Pong => log::info!("pong received"),
                Message::RouteRecall(route_recall) => remove_routes_by_gateway_and_destination(
                    inbound_message_collection.origin_id.clone(),
                    route_recall.destination_id,
                    routing_table.clone(),
                ),
            }
        }
    } else {
        // Forwarding stuff happens here
        match gateway_id {
            Some(gateway_id) => {
                let url = lookup_peer_url(&gateway_id, peering_table.clone());
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
                            Ok(message) => message_backlog.write().unwrap().push(message),
                            Err(_) => {
                                // Encountered dead url
                                // Remove all routes using that url and push message into backlog
                                remove_routes_by_url(
                                    url.clone(),
                                    routing_table.clone(),
                                    peering_table.clone(),
                                );

                                message_backlog
                                    .write()
                                    .unwrap()
                                    .push(inbound_message_collection);
                            }
                        }
                    }
                    // Know where to forward to but not how
                    // Message will have to be pulled
                    None => message_backlog
                        .write()
                        .unwrap()
                        .push(inbound_message_collection),
                }
            }
            // Don't know where to forward to
            // Will drop the message collection
            None => {
                log::error!("no route found for message collection");

                let destination_ids: Vec<String> = peering_table
                    .read()
                    .unwrap()
                    .iter()
                    .filter(|x| x.id.is_some())
                    .filter(|x| x.id != inbound_message_collection.destination_id)
                    .map(|x| x.id.clone().unwrap())
                    .collect();

                let mut message_backlog = message_backlog.write().unwrap();

                if inbound_message_collection.destination_id.is_some() {
                    for id in destination_ids.iter() {
                        message_backlog.push(MessageCollection {
                            messages: vec![Message::RouteRecall(RouteRecall {
                                destination_id: inbound_message_collection
                                    .destination_id
                                    .clone()
                                    .unwrap(),
                            })],
                            origin_id: configuration.id.clone(),
                            destination_id: Some(id.clone()),
                        });
                    }
                }

                message_backlog.push(inbound_message_collection);
            }
        }
    }

    outbound_message_collection
}
