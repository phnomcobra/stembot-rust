use crate::core::{
    backlog::push_message_collection_to_backlog,
    message::{send_message_collection_to_url, MessageCollection},
    peering::lookup_peer_url,
    routing::{recall_routes_by_destination_id, remove_routes_by_url, resolve_gateway_id},
    state::Singleton,
};

use super::promiscuous_decoding::decode_message_collection as decode_promiscuous_messages;
use super::selective_decoding::decode_message_collection as decode_selective_messages;

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

    decode_promiscuous_messages(singleton.clone(), &mut inbound_message_collection).await;

    let gateway_id = resolve_gateway_id(destination_id.clone(), singleton.clone()).await;

    if gateway_id == Some(singleton.configuration.id.clone()) {
        decode_selective_messages(
            singleton,
            &mut inbound_message_collection,
            &mut outbound_message_collection,
        )
        .await;
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
            // Issue route recalls to peers' routing tables
            None => {
                if let Some(destination_id) = inbound_message_collection.destination_id.clone() {
                    recall_routes_by_destination_id(singleton.clone(), destination_id).await;
                }

                push_message_collection_to_backlog(inbound_message_collection, singleton.clone())
                    .await;
            }
        }
    }

    outbound_message_collection
}
