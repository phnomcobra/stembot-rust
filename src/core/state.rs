use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use crate::core::{
    messaging::{MessageCollection, Ticket, TraceEvent},
    peering::Peer,
    routing::Route,
};

use crate::config::Configuration;

#[derive(Clone, Debug)]
pub struct Singleton {
    pub configuration: Configuration,
    pub peering_table: Arc<RwLock<Vec<Peer>>>,
    pub routing_table: Arc<RwLock<Vec<Route>>>,
    pub message_backlog: Arc<RwLock<Vec<MessageCollection>>>,
    pub ticket_map: Arc<RwLock<HashMap<String, Option<Ticket>>>>,
    pub trace_map: Arc<RwLock<HashMap<String, Vec<TraceEvent>>>>,
}

impl Singleton {
    pub fn new_from_cli() -> Self {
        Self {
            configuration: Configuration::new_from_cli(),
            peering_table: Arc::new(RwLock::new(vec![])),
            routing_table: Arc::new(RwLock::new(vec![])),
            message_backlog: Arc::new(RwLock::new(vec![])),
            ticket_map: Arc::new(RwLock::new(HashMap::new())),
            trace_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
