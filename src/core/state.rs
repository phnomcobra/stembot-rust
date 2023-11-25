use std::{collections::HashMap, sync::Arc, time::SystemTime};
use tokio::sync::RwLock;

use crate::{
    config::Configuration,
    core::{
        broadcasting::Broadcast, message::MessageCollection, peering::Peer, routing::Route,
        ticket::Ticket, tracing::Trace,
    },
};

#[derive(Clone, Debug)]
pub struct Singleton {
    pub configuration: Configuration,
    pub peers: Arc<RwLock<Vec<Peer>>>,
    pub routes: Arc<RwLock<Vec<Route>>>,
    pub backlog: Arc<RwLock<Vec<MessageCollection>>>,
    pub tickets: Arc<RwLock<HashMap<String, Ticket>>>,
    pub traces: Arc<RwLock<HashMap<String, Trace>>>,
    pub broadcasts: Arc<RwLock<HashMap<String, Broadcast>>>,
    pub broadcast_history: Arc<RwLock<HashMap<String, SystemTime>>>,
}

impl Singleton {
    pub fn new_from_cli() -> Self {
        Self {
            configuration: Configuration::new_from_cli(),
            peers: Arc::new(RwLock::new(vec![])),
            routes: Arc::new(RwLock::new(vec![])),
            backlog: Arc::new(RwLock::new(vec![])),
            tickets: Arc::new(RwLock::new(HashMap::new())),
            traces: Arc::new(RwLock::new(HashMap::new())),
            broadcasts: Arc::new(RwLock::new(HashMap::new())),
            broadcast_history: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
