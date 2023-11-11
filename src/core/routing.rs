use serde::{Deserialize, Serialize};

use crate::core::{
    messaging::{send_message_collection_to_url, Message, MessageCollection, RouteAdvertisement},
    processing::process_message_collection,
    state::Singleton,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Route {
    pub gateway_id: String,
    pub destination_id: String,
    pub weight: Option<usize>,
}

pub async fn resolve_gateway_id(destination_id: String, singleton: Singleton) -> Option<String> {
    let routes = singleton.routes.read().await.clone();
    let mut best_weight: Option<usize> = None;
    let mut best_gateway_id: Option<String> = None;

    for route in routes.iter().filter(|x| x.destination_id == destination_id) {
        let weight = route.weight.unwrap_or(0);

        if weight < best_weight.unwrap_or(usize::MAX) {
            best_weight = Some(weight);
            best_gateway_id = Some(route.gateway_id.clone())
        }
    }

    best_gateway_id
}

pub async fn remove_routes_by_url(url: String, singleton: Singleton) {
    let peers = singleton.peers.read().await.clone();
    let peer_ids: Vec<String> = peers
        .iter()
        .filter(|x| x.url == Some(url.clone()))
        .filter_map(|x| x.id.clone())
        .collect();
    drop(peers);

    let mut routes = singleton.routes.write().await.clone();

    let mut updated_routes: Vec<Route> = routes
        .iter()
        .filter(|x| !peer_ids.contains(&x.destination_id) || x.destination_id == x.gateway_id)
        .cloned()
        .collect();
    routes.clear();
    routes.append(&mut updated_routes);
}

pub async fn remove_routes_by_gateway_and_destination(
    gateway_id: String,
    destination_id: String,
    singleton: Singleton,
) {
    let mut routes = singleton.routes.write().await.clone();
    let mut updated_routes: Vec<Route> = routes
        .iter()
        .filter(|x| x.destination_id != destination_id && x.gateway_id != gateway_id)
        .cloned()
        .collect();
    routes.clear();
    routes.append(&mut updated_routes);
}

pub async fn advertise(singleton: Singleton) {
    let mut local_peers = singleton.peers.read().await.clone();

    let advertisement_message: Message = Message::RouteAdvertisement(
        RouteAdvertisement::from_routes(singleton.routes.read().await.clone()),
    );

    for peer in local_peers.iter_mut().filter(|x| x.url.is_some()) {
        let configuration = singleton.configuration.clone();

        let outgoing_message_collection = MessageCollection {
            messages: vec![advertisement_message.clone()],
            origin_id: configuration.id.clone(),
            destination_id: peer.id.clone(),
        };

        let url = match peer.url.clone() {
            Some(url) => url,
            None => continue,
        };

        match send_message_collection_to_url(outgoing_message_collection, url, singleton.clone())
            .await
        {
            Ok(incoming_message_collection) => {
                peer.id = Some(incoming_message_collection.origin_id.clone());

                process_message_collection(incoming_message_collection, singleton.clone()).await;
            }
            Err(error) => {
                remove_routes_by_url(peer.url.clone().unwrap(), singleton.clone()).await;
                log::error!("{}", error)
            }
        };
    }

    let mut shared_peers = singleton.peers.write().await;
    shared_peers.clear();
    shared_peers.append(&mut local_peers);
}

impl RouteAdvertisement {
    pub async fn process(&self, singleton: Singleton, origin_id: String) {
        let mut routes = singleton.routes.write().await;

        for advertised_route in self
            .routes
            .iter()
            .filter(|x| x.destination_id != singleton.configuration.id)
            .map(|x| Route {
                weight: Some(x.weight.unwrap_or(0) + 1),
                destination_id: x.destination_id.clone(),
                gateway_id: x.gateway_id.clone(),
            })
        {
            let weights: Vec<usize> = routes
                .iter()
                .filter(|x| x.destination_id == advertised_route.destination_id)
                .filter(|x| x.gateway_id == origin_id)
                .map(|x| x.weight.unwrap_or(0))
                .collect();

            match weights.iter().min() {
                None => {
                    routes.push(Route {
                        gateway_id: origin_id.clone(),
                        destination_id: advertised_route.destination_id.clone(),
                        weight: advertised_route.weight,
                    });
                }
                Some(weight) => {
                    if advertised_route.weight.unwrap() < *weight {
                        let mut indices: Vec<usize> = routes
                            .iter()
                            .enumerate()
                            .filter(|x| x.1.destination_id == advertised_route.destination_id)
                            .filter(|x| x.1.gateway_id == origin_id)
                            .map(|x| x.0)
                            .collect();

                        indices.sort_by(|a, b| b.cmp(a));

                        for i in indices.iter() {
                            routes.remove(*i);
                        }

                        routes.push(Route {
                            gateway_id: origin_id.clone(),
                            destination_id: advertised_route.destination_id.clone(),
                            weight: advertised_route.weight,
                        });
                    }
                }
            };
        }
    }

    pub fn from_routes<T: Into<Vec<Route>>>(routes: T) -> Self {
        RouteAdvertisement {
            routes: routes.into(),
        }
    }
}

pub async fn initialize_routes(singleton: Singleton) {
    let mut routes = singleton.routes.write().await;
    routes.push(Route {
        destination_id: singleton.configuration.id.clone(),
        gateway_id: singleton.configuration.id.clone(),
        weight: None,
    });
}

pub async fn age_routes(singleton: Singleton) {
    let mut routes = singleton.routes.write().await;

    let mut stale_indices: Vec<usize> = vec![];

    for (i, route) in routes
        .iter_mut()
        .enumerate()
        .filter(|x| x.1.weight.is_some())
    {
        route.weight = Some(route.weight.unwrap() + 1);
        if route.weight.unwrap() > singleton.configuration.maxrouteweight {
            stale_indices.push(i)
        }
    }

    stale_indices.sort_by(|a, b| b.cmp(a));

    for i in stale_indices {
        routes.remove(i);
    }
}
