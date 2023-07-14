use std::{
    collections::HashMap,
    ops::Add,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::Configuration,
    message::{send_message_collection_to_url, Message, MessageCollection, RouteAdvertisement},
    processor::process_message_collection,
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

        let outgoing_message_collection = MessageCollection {
            messages: vec![Message::Ping, advertisement_message.clone()],
            origin_id: configuration.id.clone(),
        };

        match send_message_collection_to_url(
            outgoing_message_collection,
            peer.url.as_ref().unwrap(),
            configuration.clone(),
        )
        .await
        {
            Ok(incoming_message_collection) => {
                peer.id = Some(incoming_message_collection.origin_id.clone());

                process_message_collection(
                    incoming_message_collection,
                    configuration.clone(),
                    peering_table.clone(),
                    routing_table.clone(),
                );
            }
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

    pub fn process(&self, routing_table: Arc<RwLock<Vec<Route>>>, origin_id: String) {
        let mut routing_table = routing_table.write().unwrap();

        for route in self.routes.iter() {
            let weights: Vec<usize> = routing_table
                .iter()
                .filter(|x| x.destination_id == route.destination_id && x.gateway_id == origin_id)
                .map(|x| x.weight)
                .collect();

            match weights.iter().min() {
                None => {
                    routing_table.push(Route {
                        gateway_id: origin_id.clone(),
                        destination_id: route.destination_id.clone(),
                        weight: route.weight,
                    });
                }
                Some(weight) => {
                    if route.weight < *weight {
                        let mut indices: Vec<usize> = routing_table
                            .iter()
                            .enumerate()
                            .filter(|x| x.1.destination_id != route.destination_id)
                            .filter(|x| x.1.gateway_id != origin_id)
                            .map(|x| x.0)
                            .collect();
                        indices.sort_by(|a, b| b.cmp(a));

                        for i in indices.iter() {
                            routing_table.remove(*i);
                        }

                        routing_table.push(Route {
                            gateway_id: origin_id.clone(),
                            destination_id: route.destination_id.clone(),
                            weight: route.weight,
                        });
                    }
                }
            };
        }

        log::warn!("{:?}", routing_table);
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
        log::warn!("detected peer: {}", &id);
    }
}
