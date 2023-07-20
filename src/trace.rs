use std::{
    fmt::{self, Display},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{
    config::Configuration,
    messaging::{Direction, TraceEvent, TraceRequest, TraceResponse},
};

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

    pub fn default() -> Self {
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

    pub fn process(&mut self, configuration: Configuration) -> TraceEvent {
        self.hop_count += 1;

        let local_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_millis(0))
            .as_millis();

        TraceEvent {
            hop_count: self.hop_count,
            request_id: self.request_id.clone(),
            local_time,
            id: configuration.id,
            direction: Direction::Outbound,
        }
    }
}

impl Display for TraceEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "trace {}: {}: hop: {}, time: {}, {} ",
            self.request_id, self.id, self.hop_count, self.local_time, self.direction
        )
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

    pub fn process(&mut self, configuration: Configuration) -> TraceEvent {
        self.hop_count += 1;

        let local_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_millis(0))
            .as_millis();

        TraceEvent {
            hop_count: self.hop_count,
            request_id: self.request_id.clone(),
            local_time,
            id: configuration.id,
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
