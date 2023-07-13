use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::Configuration,
    message::{send_message_collection_to_url, Message, MessageCollection},
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Route {
    pub gateway_id: String,
    pub destination_id: String,
    pub weight: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteTable {
    pub routes: HashMap<String, Vec<Route>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Peer {
    pub id: Option<String>,
    pub url: Option<String>,
    pub polling: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerTable {
    pub peers: Vec<Peer>,
}

pub async fn advertise(
    configuration: Configuration,
    peering_table: Arc<RwLock<Vec<Peer>>>,
    routing_table: Arc<RwLock<Vec<Route>>>,
) {
    let mut local_peering_table = peering_table.read().unwrap().clone();

    for peer in local_peering_table.iter_mut().filter(|x| x.url.is_some()) {
        let configuration = configuration.clone();

        let message = Message::Ping;
        let message_collection = MessageCollection {
            messages: vec![message],
            origin_id: configuration.id.clone(),
        };

        match send_message_collection_to_url(
            message_collection,
            peer.url.as_ref().unwrap(),
            configuration,
        )
        .await
        {
            Ok(message_collection) => peer.id = Some(message_collection.origin_id.clone()),
            Err(error) => log::error!("{}", error),
        };
    }

    let mut shared_peering_table = peering_table.write().unwrap();
    shared_peering_table.clear();
    shared_peering_table.append(&mut local_peering_table);
}

pub fn initialize_peers(configuration: Configuration, peering_table: Arc<RwLock<Vec<Peer>>>) {
    let mut peering_table = peering_table.write().unwrap();
    for peer in configuration.clone().peer.into_values() {
        peering_table.push(Peer {
            id: None,
            url: peer.url.clone(),
            polling: peer.polling,
        })
    }
}
