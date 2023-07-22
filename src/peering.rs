use serde::{Deserialize, Serialize};

use crate::state::Singleton;

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
    let mut peering_table = singleton.peering_table.write().await;
    for peer in singleton
        .configuration
        .clone()
        .peer
        .into_values()
        .filter(|x| x.url.is_some())
    {
        peering_table.push(Peer {
            id: None,
            url: peer.url.clone(),
            polling: peer.polling,
        })
    }
}

pub async fn touch_peer(id: &String, singleton: Singleton) {
    let peering_table_read = singleton.peering_table.read().await;

    let peers: Vec<&Peer> = peering_table_read
        .iter()
        .filter(|x| x.id == Some(id.to_string()))
        .collect();
    let present = !peers.is_empty();

    drop(peers);
    drop(peering_table_read);

    if !present {
        let mut peering_table_write = singleton.peering_table.write().await;
        peering_table_write.push(Peer {
            id: Some(id.to_string()),
            url: None,
            polling: false,
        });
    }
}

pub async fn lookup_peer_url(id: &String, singleton: Singleton) -> Option<String> {
    let peering_table = singleton.peering_table.read().await;

    let peers: Vec<Peer> = peering_table
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
