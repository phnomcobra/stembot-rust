use std::sync::{Arc, RwLock};

use crate::{
    config::Configuration,
    messaging::{send_message_collection_to_url, Message, MessageCollection},
    peering::{check_peer, lookup_peer_url, Peer},
    routing::{remove_routes_by_url, resolve_gateway_id, Route},
};

pub async fn process_message_collection<T: Into<MessageCollection>, U: Into<Configuration>>(
    inbound_message_collection: T,
    configuration: U,
    peering_table: Arc<RwLock<Vec<Peer>>>,
    routing_table: Arc<RwLock<Vec<Route>>>,
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
                            Ok(_message) => {}
                            Err(_) => {
                                remove_routes_by_url(
                                    url.clone(),
                                    routing_table.clone(),
                                    peering_table.clone(),
                                );

                                /*
                                process_message_collection(
                                    inbound_message_collection,
                                    configuration.clone(),
                                    peering_table.clone(),
                                    routing_table.clone(),
                                )
                                .await;
                                */
                            }
                        }
                    }
                    // Know where to forward to but not how
                    None => {}
                }
            }
            // Don't know where to forward to
            None => {}
        }
    }

    outbound_message_collection
}
