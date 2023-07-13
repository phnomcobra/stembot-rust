use std::{
    collections::HashMap,
    ops::Add,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::Configuration,
    message::{send_message_collection_to_url, Message, MessageCollection, RouteAdvertisement},
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
    let static_peering_table = peering_table.read().unwrap().clone();

    let advertisement_message: Message =
        Message::RouteAdvertisement(RouteAdvertisement::from_peers_and_routes(
            routing_table.read().unwrap().clone(),
            static_peering_table.clone(),
        ));

    for peer in local_peering_table.iter_mut().filter(|x| x.url.is_some()) {
        let configuration = configuration.clone();

        let message_collection = MessageCollection {
            messages: vec![Message::Ping, advertisement_message.clone()],
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

impl RouteAdvertisement {
    pub fn default() -> Self {
        Self { routes: vec![] }
    }

    pub fn from_peers<T: Into<Vec<Peer>>>(peers: T) -> Self {
        let peers = peers.into();

        let mut advertisement = Self::default();

        for peer in peers.iter() {
            let id = match &peer.id {
                Some(id) => id.clone(),
                None => continue,
            };

            advertisement.routes.push(Route {
                destination_id: id.clone(),
                gateway_id: id,
                weight: 0,
            })
        }

        advertisement
    }

    pub fn from_routes<T: Into<Vec<Route>>>(routes: T) -> Self {
        let routes = routes.into();

        let mut advertisement = Self::default();

        for route in routes.iter() {
            advertisement.routes.push(Route {
                destination_id: route.destination_id.clone(),
                gateway_id: route.gateway_id.clone(),
                weight: &route.weight + 1,
            })
        }

        advertisement
    }

    pub fn from_peers_and_routes<T: Into<Vec<Route>>, U: Into<Vec<Peer>>>(
        routes: T,
        peers: U,
    ) -> Self {
        Self::from_peers(peers.into().clone()) + Self::from_routes(routes.into().clone())
    }
}

impl Add for RouteAdvertisement {
    type Output = Self;

    fn add(self, other: RouteAdvertisement) -> Self {
        let mut routes: Vec<Route> = self.routes.clone();
        let mut others: Vec<Route> = other.routes.clone();
        routes.append(&mut others);
        RouteAdvertisement { routes }
    }
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
