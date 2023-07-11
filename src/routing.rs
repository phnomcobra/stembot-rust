use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
    pub url: String,
    pub polling: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerTable {
    pub peers: Vec<Peer>,
}
