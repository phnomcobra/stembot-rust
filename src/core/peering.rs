use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::core::state::Singleton;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Peer {
    pub id: Option<String>,
    pub url: Option<String>,
    pub polling: bool,
}

impl Display for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let polling = if self.polling { "polling" } else { "" };
        write!(f, "{:?} {:?} {polling}", self.id, self.url)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerTable {
    pub peers: Vec<Peer>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PeerQuery {
    pub peers: Option<Vec<Peer>>,
}

pub async fn initialize_peers(singleton: Singleton) {
    let mut peers = singleton.peers.write().await;
    for peer in singleton
        .configuration
        .clone()
        .peer
        .into_values()
        .filter(|x| x.url.is_some())
    {
        peers.push(Peer {
            id: None,
            url: peer.url.clone(),
            polling: peer.polling,
        })
    }
}

pub async fn lookup_peer_url(id: &String, singleton: Singleton) -> Option<String> {
    let peers = singleton.peers.read().await;

    let peers: Vec<Peer> = peers
        .iter()
        .filter(|x| Some(id.to_string()) == x.id)
        .filter(|x| x.url.is_some())
        .cloned()
        .collect();

    match peers.first() {
        Some(peer) => peer.url.clone(),
        None => None,
    }
}
