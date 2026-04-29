// use std::{collections::HashMap, sync::Arc, time::SystemTime};
// use tokio::sync::RwLock;

use crate::{
    models::config::Configuration,
};

#[derive(Clone, Debug, Default)]
pub struct Singleton {
    pub configuration: Configuration
    // pub peers: Arc<RwLock<Vec<Peer>>>,
    // pub routes: Arc<RwLock<Vec<Route>>>,
}
