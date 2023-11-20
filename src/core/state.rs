use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

use crate::{
    config::Configuration,
    core::{
        messaging::MessageCollection, peering::Peer, routing::Route, ticketing::TicketSlice,
        tracing::TraceEvent,
    },
};

#[derive(Clone, Debug)]
pub struct Singleton {
    pub configuration: Configuration,
    pub peers: Arc<RwLock<Vec<Peer>>>,
    pub routes: Arc<RwLock<Vec<Route>>>,
    pub backlog: Arc<RwLock<Vec<MessageCollection>>>,
    pub tickets: Arc<RwLock<HashMap<String, TicketSlice>>>,
    pub traces: Arc<RwLock<HashMap<String, Vec<TraceEvent>>>>,
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
        }
    }
}
