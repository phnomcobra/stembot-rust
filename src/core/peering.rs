use serde::{Deserialize, Serialize};

use crate::core::state::Singleton;

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

pub async fn touch_peer(id: &String, singleton: Singleton) {
    let peers_read = singleton.peers.read().await;

    let peers: Vec<&Peer> = peers_read
        .iter()
        .filter(|x| x.id == Some(id.to_string()))
        .collect();
    let present = !peers.is_empty();

    drop(peers);
    drop(peers_read);

    if !present {
        let mut peers_write = singleton.peers.write().await;
        peers_write.push(Peer {
            id: Some(id.to_string()),
            url: None,
            polling: false,
        });
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
