use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use crate::{
    config::Configuration, messaging::MessageCollection, peering::Peer,
    processing::process_message_collection, routing::Route,
};

pub async fn process_backlog<T: Into<Configuration>>(
    configuration: T,
    peering_table: Arc<RwLock<Vec<Peer>>>,
    routing_table: Arc<RwLock<Vec<Route>>>,
    message_backlog: Arc<RwLock<Vec<MessageCollection>>>,
) {
    let configuration = configuration.into();

    let mut local_message_backlog = message_backlog.write().await.clone();

    let mut message_buckets: HashMap<Option<String>, MessageCollection> = HashMap::default();

    // Condense message collections with common destination options
    while !local_message_backlog.is_empty() {
        let mut message_collection = local_message_backlog.pop().unwrap();

        match message_buckets.contains_key(&message_collection.destination_id) {
            true => {
                message_buckets
                    .entry(message_collection.destination_id.clone())
                    .and_modify(|x| x.messages.append(&mut message_collection.messages));
            }
            false => {
                message_buckets.insert(
                    message_collection.destination_id.clone(),
                    message_collection,
                );
            }
        }
    }

    drop(local_message_backlog);

    for outbound_message_collection in message_buckets.values() {
        let inbound_message_collection = process_message_collection(
            outbound_message_collection.clone(),
            configuration.clone(),
            peering_table.clone(),
            routing_table.clone(),
            message_backlog.clone(),
        )
        .await;

        if !inbound_message_collection.messages.is_empty() {
            message_backlog
                .write()
                .await
                .push(inbound_message_collection);
        }
    }
}
