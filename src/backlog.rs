use std::collections::HashMap;

use actix_web::rt::spawn;

use crate::{
    messaging::{BacklogRequest, BacklogResponse, Message, MessageCollection},
    processing::process_message_collection,
    routing::resolve_gateway_id,
    state::Singleton,
};

pub async fn poll_backlogs(singleton: Singleton) {
    let peering_table = singleton.peering_table.read().await;
    let backlog_request = BacklogRequest {
        gateway_id: singleton.configuration.id.clone(),
    };

    for peer in peering_table
        .iter()
        .filter(|x| x.polling)
        .filter(|x| x.url.is_some())
    {
        let message_collection = MessageCollection {
            messages: vec![Message::BacklogRequest(backlog_request.clone())],
            origin_id: singleton.configuration.id.clone(),
            destination_id: peer.id.clone(),
        };

        push_message_collection_to_backlog(message_collection, singleton.clone()).await
    }
}

pub async fn push_message_collection_to_backlog(
    message_collection: MessageCollection,
    singleton: Singleton,
) {
    if !message_collection.messages.is_empty() {
        singleton
            .message_backlog
            .write()
            .await
            .push(message_collection);
    }
}

pub async fn request_backlog(
    singleton: Singleton,
    backlog_request: BacklogRequest,
) -> BacklogResponse {
    let mut message_backlog = singleton.message_backlog.write().await;
    let mut backlog_indices_to_remove: Vec<usize> = vec![];
    let mut backlog_response = BacklogResponse {
        message_collections: vec![],
    };

    for (i, message_collection) in message_backlog.iter().enumerate() {
        let resovled_gateway_id = resolve_gateway_id(
            message_collection
                .destination_id
                .clone()
                .unwrap_or_else(|| singleton.configuration.id.clone()),
            singleton.clone(),
        )
        .await;

        if resovled_gateway_id == Some(backlog_request.gateway_id.clone()) {
            backlog_indices_to_remove.push(i);
            backlog_response
                .message_collections
                .push(message_collection.clone())
        }
    }

    backlog_indices_to_remove.sort_by(|a, b| b.cmp(a));
    for i in backlog_indices_to_remove {
        message_backlog.remove(i);
    }

    backlog_response
}

pub async fn process_backlog(singleton: Singleton) {
    let mut local_message_backlog = singleton.message_backlog.write().await;

    let mut message_buckets: HashMap<Option<String>, MessageCollection> = HashMap::default();

    // Condense message collections with common destination options
    while !local_message_backlog.is_empty() {
        let mut message_collection = local_message_backlog.pop().unwrap();

        if message_collection.messages.is_empty() {
            continue;
        }

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

    // Process condesnsed message collections
    for outbound_message_collection in message_buckets.values() {
        spawn({
            let singleton = singleton.clone();
            let outbound_message_collection = outbound_message_collection.clone();

            async move {
                let inbound_message_collection = process_message_collection(
                    outbound_message_collection.clone(),
                    singleton.clone(),
                )
                .await;

                if !inbound_message_collection.messages.is_empty() {
                    singleton
                        .message_backlog
                        .write()
                        .await
                        .push(inbound_message_collection);
                }
            }
        });
    }
}
