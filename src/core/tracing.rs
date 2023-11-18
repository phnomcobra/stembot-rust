use std::{
    fmt::{self, Display},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::core::state::Singleton;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TraceRequest {
    pub hop_count: usize,
    pub request_id: String,
    pub start_time: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TraceResponse {
    pub hop_count: usize,
    pub request_id: String,
    pub start_time: u128,
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
    pub period: Option<u64>,
    pub request_id: Option<String>,
    pub destination_id: String,
}

impl Default for TraceRequest {
    fn default() -> Self {
        let request_id: String = rand::random::<usize>().to_string();
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_millis(0))
            .as_millis();

        Self {
            hop_count: 0,
            request_id,
            start_time,
        }
    }
}

impl TraceRequest {
    pub fn new(request_id: String) -> Self {
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_millis(0))
            .as_millis();

        Self {
            hop_count: 0,
            request_id,
            start_time,
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
            start_time: trace_request.start_time,
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
        let end_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_millis(0))
            .as_millis();

        write!(
            f,
            "trace response {}: hops: {}, elapsed time {} ms",
            self.request_id,
            self.hop_count,
            end_time - self.start_time
        )
    }
}
