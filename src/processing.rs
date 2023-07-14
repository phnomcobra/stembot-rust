use std::sync::{Arc, RwLock};

use crate::{
    config::Configuration,
    messaging::{Message, MessageCollection},
    peering::{check_peer, Peer},
    routing::Route,
};

pub fn process_message_collection<T: Into<MessageCollection>, U: Into<Configuration>>(
    inbound_message_collection: T,
    configuration: U,
    peering_table: Arc<RwLock<Vec<Peer>>>,
    routing_table: Arc<RwLock<Vec<Route>>>,
) -> MessageCollection {
    let configuration = configuration.into();

    let inbound_message_collection = inbound_message_collection.into();

    check_peer(&inbound_message_collection.origin_id, peering_table);

    let mut outbound_message_collection = MessageCollection {
        messages: vec![],
        origin_id: configuration.id.clone(),
    };

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

    outbound_message_collection
}
