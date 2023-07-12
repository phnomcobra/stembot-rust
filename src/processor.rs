use std::sync::{Arc, RwLock};

use crate::{
    config::Configuration,
    message::{Message, MessageCollection},
    routing::Peer,
};

pub fn process_message_collection<T: Into<MessageCollection>, U: Into<Configuration>>(
    inbound_message_collection: T,
    configuration: U,
    peering_table: Arc<RwLock<Vec<Peer>>>,
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
            _ => log::warn!("processor not implemented for {:?}", &message),
        }
    }

    outbound_message_collection
}

fn check_peer(id: &String, peering_table: Arc<RwLock<Vec<Peer>>>) {
    let peering_table_read = peering_table.read().unwrap();

    let peers: Vec<&Peer> = peering_table_read
        .iter()
        .filter(|x| x.id == Some(id.to_string()))
        .collect();
    let present = peers.len() > 0;

    drop(peers);
    drop(peering_table_read);

    if !present {
        let mut peering_table_write = peering_table.write().unwrap();
        peering_table_write.push(Peer {
            id: Some(id.to_string()),
            url: None,
            polling: false,
        });
        log::warn!("detected peer: {}", &id);
    }
}
