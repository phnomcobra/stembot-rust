use std::{
    collections::HashSet,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::core::{
    backlog::push_message_collection_to_backlog,
    message::{Message, MessageCollection},
    state::Singleton,
};

use super::{BroadcastMessage, BroadcastRequest, BroadcastResponse};

pub async fn poll_broadcasts(singleton: Singleton) {
    let mut history = singleton.broadcast_history.write().await;

    for (broadcast_id, system_time) in history.clone().iter() {
        match SystemTime::now().duration_since(*system_time) {
            Ok(duration) => {
                if duration.as_millis() > singleton.configuration.broadcastexpiration.into() {
                    history.remove(broadcast_id);
                    log::info!("aged out broadcast history for broadcast {broadcast_id}");
                }
            }
            Err(error) => {
                log::warn!("could not determine age for broadcast {broadcast_id}: {error}");
                history.remove(broadcast_id);
            }
        }
    }
}

pub async fn process_broadcast_request(singleton: Singleton, broadcast_request: BroadcastRequest) {
    let mut history = singleton.broadcast_history.write().await;
    let routes = singleton.routes.read().await;

    if !history.contains_key(&broadcast_request.request_id) {
        history.insert(broadcast_request.request_id.clone(), SystemTime::now());

        // forward broadcast request
        for gateway_id in
            HashSet::<String>::from_iter(routes.iter().map(|route| route.gateway_id.clone()))
        {
            let message_collection = MessageCollection {
                messages: vec![Message::BroadcastRequest(broadcast_request.clone())],
                origin_id: singleton.configuration.id.clone(),
                destination_id: Some(gateway_id.clone()),
            };

            push_message_collection_to_backlog(message_collection, singleton.clone()).await;
        }

        // process broadcast request
        if let (Some(broadcast), Some(destination_id)) = (
            process_broadcast_message(singleton.clone(), broadcast_request.broadcast_message).await,
            broadcast_request.origin_id.clone(),
        ) {
            let broadcast_response = BroadcastResponse {
                request_id: broadcast_request.request_id.clone(),
                local_time: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::from_millis(0))
                    .as_millis(),
                broadcast,
            };

            let message_collection = MessageCollection {
                messages: vec![Message::BroadcastResponse(broadcast_response)],
                origin_id: singleton.configuration.id.clone(),
                destination_id: Some(destination_id),
            };

            push_message_collection_to_backlog(message_collection, singleton.clone()).await;
        }
    }
}

pub async fn process_broadcast_response(
    singleton: Singleton,
    broadcast_response: BroadcastResponse,
    origin_id: String,
) {
    let mut broadcasts = singleton.broadcasts.write().await;

    if let Some(broadcast_state) = broadcasts.get_mut(&broadcast_response.request_id) {
        broadcast_state
            .responses
            .insert(origin_id, broadcast_response);
    } else {
        log::warn!(
            "unsolicited broadcast response received: {}, from {origin_id}",
            broadcast_response.request_id
        );
    }
}

async fn process_broadcast_message(
    _singleton: Singleton,
    broadcast_message: BroadcastMessage,
) -> Option<BroadcastMessage> {
    match broadcast_message {
        BroadcastMessage::Ping => {
            log::warn!("broadcast ping received");
            Some(BroadcastMessage::Pong)
        }
        BroadcastMessage::Pong => {
            log::warn!("broadcast pong received");
            None
        }
    }
}
