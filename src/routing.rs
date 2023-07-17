use std::{
    ops::Add,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::Configuration,
    messaging::{send_message_collection_to_url, Message, MessageCollection, RouteAdvertisement},
    peering::Peer,
    processing::process_message_collection,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Route {
    pub gateway_id: String,
    pub destination_id: String,
    pub weight: Option<usize>,
}

pub fn resolve_gateway_id(
    destination_id: String,
    routing_table: Arc<RwLock<Vec<Route>>>,
) -> Option<String> {
    let routing_table = routing_table.read().unwrap().clone();
    let mut best_weight: Option<usize> = None;
    let mut best_gateway_id: Option<String> = None;

    for route in routing_table
        .iter()
        .filter(|x| x.destination_id == destination_id)
    {
        let weight = route.weight.unwrap_or(0);

        if best_weight.is_none() {
            best_weight = Some(weight);
            best_gateway_id = Some(route.gateway_id.clone())
        } else if weight < best_weight.unwrap() {
            best_weight = Some(weight);
            best_gateway_id = Some(route.gateway_id.clone())
        }
    }

    best_gateway_id
}

pub fn remove_routes_by_url(
    url: String,
    routing_table: Arc<RwLock<Vec<Route>>>,
    peering_table: Arc<RwLock<Vec<Peer>>>,
) {
    let peering_table = peering_table.read().unwrap().clone();
    let peer_ids: Vec<String> = peering_table
        .iter()
        .filter(|x| x.url == Some(url.clone()))
        .filter(|x| x.id.is_some())
        .map(|x| x.id.clone().unwrap())
        .collect();
    drop(peering_table);

    let mut routing_table = routing_table.write().unwrap().clone();

    let mut updated_routing_table: Vec<Route> = routing_table
        .iter()
        .filter(|x| !peer_ids.contains(&x.destination_id))
        .map(|x| x.clone())
        .collect();
    routing_table.clear();
    routing_table.append(&mut updated_routing_table);
}

pub fn remove_routes_by_gateway_and_destination(
    gateway_id: String,
    destination_id: String,
    routing_table: Arc<RwLock<Vec<Route>>>,
) {
    let mut routing_table = routing_table.write().unwrap().clone();
    let mut updated_routing_table: Vec<Route> = routing_table
        .iter()
        .filter(|x| x.destination_id != destination_id && x.gateway_id != gateway_id)
        .map(|x| x.clone())
        .collect();
    routing_table.clear();
    routing_table.append(&mut updated_routing_table);
}

pub async fn advertise(
    configuration: Configuration,
    peering_table: Arc<RwLock<Vec<Peer>>>,
    routing_table: Arc<RwLock<Vec<Route>>>,
    message_backlog: Arc<RwLock<Vec<MessageCollection>>>,
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
            destination_id: peer.id.clone(),
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
                    message_backlog.clone(),
                )
                .await;
            }
            Err(error) => {
                remove_routes_by_url(
                    peer.url.clone().unwrap(),
                    routing_table.clone(),
                    peering_table.clone(),
                );
                log::error!("{}", error)
            }
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

    pub fn process(
        &self,
        configuration: Configuration,
        routing_table: Arc<RwLock<Vec<Route>>>,
        origin_id: String,
    ) {
        let mut routing_table = routing_table.write().unwrap();

        for route in self
            .routes
            .iter()
            .filter(|x| x.destination_id != configuration.id)
            .map(|x| Route {
                weight: Some(x.weight.unwrap_or(0)),
                destination_id: x.destination_id.clone(),
                gateway_id: x.gateway_id.clone(),
            })
        {
            let weights: Vec<usize> = routing_table
                .iter()
                .filter(|x| x.destination_id == route.destination_id)
                .filter(|x| x.gateway_id == origin_id)
                .filter(|x| x.weight.is_some())
                .map(|x| x.weight.unwrap())
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
                    if route.weight.unwrap() < *weight {
                        let mut indices: Vec<usize> = routing_table
                            .iter()
                            .enumerate()
                            .filter(|x| x.1.destination_id == route.destination_id)
                            .filter(|x| x.1.gateway_id == origin_id)
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
                weight: Some(1),
            })
        }

        advertisement
    }

    pub fn from_routes<T: Into<Vec<Route>>>(routes: T) -> Self {
        let routes = routes.into();

        let mut advertisement = Self::default();

        for route in routes.iter() {
            let weight = match route.weight {
                Some(x) => Some(x + 1),
                None => None,
            };

            advertisement.routes.push(Route {
                destination_id: route.destination_id.clone(),
                gateway_id: route.gateway_id.clone(),
                weight,
            });
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

pub fn initialize_routes(configuration: Configuration, routing_table: Arc<RwLock<Vec<Route>>>) {
    let mut routing_table = routing_table.write().unwrap();
    routing_table.push(Route {
        destination_id: configuration.id.clone(),
        gateway_id: configuration.id.clone(),
        weight: None,
    });
}

pub async fn age_routes(configuration: Configuration, routing_table: Arc<RwLock<Vec<Route>>>) {
    let mut routing_table = routing_table.write().unwrap();

    let mut stale_indices: Vec<usize> = vec![];

    for (i, route) in routing_table
        .iter_mut()
        .enumerate()
        .filter(|x| x.1.weight.is_some())
    {
        route.weight = Some(route.weight.unwrap() + 1);
        if route.weight.unwrap() > configuration.maxrouteweight {
            stale_indices.push(i)
        }
    }

    stale_indices.sort_by(|a, b| b.cmp(a));

    for i in stale_indices {
        routing_table.remove(i);
    }
}
