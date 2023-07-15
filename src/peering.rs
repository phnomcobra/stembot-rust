use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::config::Configuration;

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

pub fn check_peer(id: &String, peering_table: Arc<RwLock<Vec<Peer>>>) {
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
    }
}

pub fn lookup_peer_url(id: &String, peering_table: Arc<RwLock<Vec<Peer>>>) -> Option<String> {
    let peering_table = peering_table.read().unwrap();

    for peer in peering_table
        .iter()
        .filter(|x| Some(id.to_string()) == x.id)
        .filter(|x| x.url.is_some())
    {
        return peer.url.clone();
    }

    None
}
