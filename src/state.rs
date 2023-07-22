use std::sync::Arc;

use tokio::sync::RwLock;

use crate::{config::Configuration, messaging::MessageCollection, peering::Peer, routing::Route};

#[derive(Clone, Debug)]
pub struct Singleton {
    pub configuration: Configuration,
    pub peering_table: Arc<RwLock<Vec<Peer>>>,
    pub routing_table: Arc<RwLock<Vec<Route>>>,
    pub message_backlog: Arc<RwLock<Vec<MessageCollection>>>,
}

impl Singleton {
    pub fn new_from_cli() -> Self {
        Self {
            configuration: Configuration::new_from_cli(),
            peering_table: Arc::new(RwLock::new(vec![])),
            routing_table: Arc::new(RwLock::new(vec![])),
            message_backlog: Arc::new(RwLock::new(vec![])),
        }
    }
}
