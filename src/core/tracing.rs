use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::core::state::Singleton;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TraceRequest {
    pub hop_count: usize,
    pub request_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TraceResponse {
    pub hop_count: usize,
    pub request_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Direction {
    Outbound,
    Inbound,
}

impl Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Inbound => write!(f, "inbound"),
            Self::Outbound => write!(f, "outbound"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TraceEvent {
    pub hop_count: usize,
    pub request_id: String,
    pub local_time: u128,
    pub id: String,
    pub direction: Direction,
}

impl Display for TraceEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} {}",
            &self.local_time, &self.hop_count, &self.id, &self.direction
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Trace {
    pub events: Vec<TraceEvent>,
    pub request_id: Option<String>,
    pub destination_id: String,
    pub start_time: Option<u128>,
    pub stop_time: Option<u128>,
}

impl Display for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let duration_ms =
            if let (Some(start_time), Some(stop_time)) = (self.start_time, self.stop_time) {
                Some(stop_time - start_time)
            } else {
                None
            };

        write!(
            f,
            "destination: {}, duration msec: {duration_ms:?}, hops: {}",
            &self.destination_id,
            &self.events.len(),
        )
    }
}

impl Default for TraceRequest {
    fn default() -> Self {
        Self {
            hop_count: 0,
            request_id: rand::random::<usize>().to_string(),
        }
    }
}

impl TraceRequest {
    pub fn new(request_id: String) -> Self {
        Self {
            hop_count: 0,
            request_id,
        }
    }

    pub fn process(&mut self, singleton: Singleton) -> TraceEvent {
        self.hop_count += 1;

        let local_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_millis(0))
            .as_millis();

        TraceEvent {
            hop_count: self.hop_count,
            request_id: self.request_id.clone(),
            local_time,
            id: singleton.configuration.id,
            direction: Direction::Outbound,
        }
    }
}

impl TraceResponse {
    pub fn from(trace_request: TraceRequest) -> Self {
        Self {
            hop_count: trace_request.hop_count,
            request_id: trace_request.request_id.clone(),
        }
    }

    pub fn process(&mut self, singleton: Singleton) -> TraceEvent {
        self.hop_count += 1;

        let local_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_millis(0))
            .as_millis();

        TraceEvent {
            hop_count: self.hop_count,
            request_id: self.request_id.clone(),
            local_time,
            id: singleton.configuration.id,
            direction: Direction::Inbound,
        }
    }
}

impl Display for TraceResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "trace response {}: hops: {}",
            self.request_id, self.hop_count,
        )
    }
}
