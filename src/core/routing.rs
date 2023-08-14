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
    let routing_table = singleton.routing_table.read().await.clone();
    let mut best_weight: Option<usize> = None;
    let mut best_gateway_id: Option<String> = None;

    for route in routing_table
        .iter()
        .filter(|x| x.destination_id == destination_id)
    {
        let weight = route.weight.unwrap_or(0);

        if weight < best_weight.unwrap_or(usize::MAX) {
            best_weight = Some(weight);
            best_gateway_id = Some(route.gateway_id.clone())
        }
    }

    best_gateway_id
}

pub async fn remove_routes_by_url(url: String, singleton: Singleton) {
    let peering_table = singleton.peering_table.read().await.clone();
    let peer_ids: Vec<String> = peering_table
        .iter()
        .filter(|x| x.url == Some(url.clone()))
        .filter_map(|x| x.id.clone())
        .collect();
    drop(peering_table);

    let mut routing_table = singleton.routing_table.write().await.clone();

    let mut updated_routing_table: Vec<Route> = routing_table
        .iter()
        .filter(|x| !peer_ids.contains(&x.destination_id) || x.destination_id == x.gateway_id)
        .cloned()
        .collect();
    routing_table.clear();
    routing_table.append(&mut updated_routing_table);
}

pub async fn remove_routes_by_gateway_and_destination(
    gateway_id: String,
    destination_id: String,
    singleton: Singleton,
) {
    let mut routing_table = singleton.routing_table.write().await.clone();
    let mut updated_routing_table: Vec<Route> = routing_table
        .iter()
        .filter(|x| x.destination_id != destination_id && x.gateway_id != gateway_id)
        .cloned()
        .collect();
    routing_table.clear();
    routing_table.append(&mut updated_routing_table);
}

pub async fn advertise(singleton: Singleton) {
    let mut local_peering_table = singleton.peering_table.read().await.clone();

    let advertisement_message: Message = Message::RouteAdvertisement(
        RouteAdvertisement::from_routes(singleton.routing_table.read().await.clone()),
    );

    for peer in local_peering_table.iter_mut().filter(|x| x.url.is_some()) {
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

    let mut shared_peering_table = singleton.peering_table.write().await;
    shared_peering_table.clear();
    shared_peering_table.append(&mut local_peering_table);
}

impl RouteAdvertisement {
    pub fn default() -> Self {
        Self { routes: vec![] }
    }

    pub async fn process(&self, singleton: Singleton, origin_id: String) {
        let mut routing_table = singleton.routing_table.write().await;

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
            let weights: Vec<usize> = routing_table
                .iter()
                .filter(|x| x.destination_id == advertised_route.destination_id)
                .filter(|x| x.gateway_id == origin_id)
                .map(|x| x.weight.unwrap_or(0))
                .collect();

            match weights.iter().min() {
                None => {
                    routing_table.push(Route {
                        gateway_id: origin_id.clone(),
                        destination_id: advertised_route.destination_id.clone(),
                        weight: advertised_route.weight,
                    });
                }
                Some(weight) => {
                    if advertised_route.weight.unwrap() < *weight {
                        let mut indices: Vec<usize> = routing_table
                            .iter()
                            .enumerate()
                            .filter(|x| x.1.destination_id == advertised_route.destination_id)
                            .filter(|x| x.1.gateway_id == origin_id)
                            .map(|x| x.0)
                            .collect();

                        indices.sort_by(|a, b| b.cmp(a));

                        for i in indices.iter() {
                            routing_table.remove(*i);
                        }

                        routing_table.push(Route {
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
        let routes = routes.into();
        let mut advertisement = Self::default();
        advertisement.routes = routes;
        advertisement
    }
}

pub async fn initialize_routes(singleton: Singleton) {
    let mut routing_table = singleton.routing_table.write().await;
    routing_table.push(Route {
        destination_id: singleton.configuration.id.clone(),
        gateway_id: singleton.configuration.id.clone(),
        weight: None,
    });
}

pub async fn age_routes(singleton: Singleton) {
    let mut routing_table = singleton.routing_table.write().await;

    let mut stale_indices: Vec<usize> = vec![];

    for (i, route) in routing_table
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
        routing_table.remove(i);
    }
}
