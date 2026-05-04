use serde_json::json;
use uuid::Uuid;

use crate::dao::kvstore::KVStore;

/// Log level, mirroring Python's `LogLevel` IntEnum.
#[derive(Clone, Debug, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl Default for LogLevel {
    fn default() -> Self { LogLevel::Info }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug    => write!(f, "DEBUG"),
            LogLevel::Info     => write!(f, "INFO"),
            LogLevel::Warning  => write!(f, "WARNING"),
            LogLevel::Error    => write!(f, "ERROR"),
            LogLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "DEBUG"            => Ok(LogLevel::Debug),
            "INFO"             => Ok(LogLevel::Info),
            "WARNING" | "WARN" => Ok(LogLevel::Warning),
            "ERROR"            => Ok(LogLevel::Error),
            "CRITICAL"         => Ok(LogLevel::Critical),
            _ => Err(format!("unknown log level '{s}'; expected DEBUG, INFO, WARNING, ERROR, or CRITICAL")),
        }
    }
}

/// Configuration settings for the StemBot distributed agent framework.
/// All values are loaded from the kvstore collection; defaults are seeded on first run.
#[derive(Clone, Debug)]
pub struct Config {
    /// Unique identifier for this agent (UUID string, 1-36 characters).
    pub agtuuid: String,
    /// Number of worker processes (default: 2).
    pub workers: u32,
    /// IP address or hostname to bind the HTTP server (default: 0.0.0.0).
    pub socket_host: String,
    /// Port number for the HTTP server (default: 8080).
    pub socket_port: u16,
    /// SHA-256 hex digest of the secret key (default: sha256("changeme")).
    pub secret_digest: String,
    /// URL where the control client can reach this agent.
    pub client_control_url: String,
    /// Log level for the application logger (default: INFO).
    pub log_level_app: LogLevel,
    /// Log level for the API/web framework logger (default: WARNING).
    pub log_level_api: LogLevel,
    /// Seconds before an unresponsive peer is considered dead (default: 60).
    pub peer_timeout_secs: u32,
    /// Seconds between peer refresh cycles (default: 30).
    pub peer_refresh_secs: u32,
    /// Maximum weight value for routes in routing decisions (default: 600).
    pub max_weight: u32,
    /// Seconds before a ticket is considered expired (default: 600).
    pub ticket_timeout_secs: u32,
    /// Seconds before a pending message is discarded (default: 600).
    pub message_timeout_secs: u32,
}

impl Config {
    /// Load all config values from the kvstore, seeding defaults for any missing keys.
    pub fn load() -> Self {
        let store = KVStore::new(None).expect("failed to open kvstore");

        macro_rules! kv_str {
            ($key:expr, $default:expr) => {
                store
                    .get($key, Some(json!($default)))
                    .unwrap_or(json!($default))
                    .as_str()
                    .unwrap_or($default)
                    .to_string()
            };
        }
        macro_rules! kv_u32 {
            ($key:expr, $default:expr) => {
                store
                    .get($key, Some(json!($default)))
                    .unwrap_or(json!($default))
                    .as_u64()
                    .unwrap_or($default as u64) as u32
            };
        }
        macro_rules! kv_level {
            ($key:expr, $default:expr) => {{
                let s = store
                    .get($key, Some(json!($default)))
                    .unwrap_or(json!($default))
                    .as_str()
                    .unwrap_or($default)
                    .to_string();
                s.parse::<LogLevel>().unwrap_or_default()
            }};
        }

        let agtuuid_default = Uuid::new_v4().to_string();
        let agtuuid = store
            .get("agtuuid", Some(json!(agtuuid_default.clone())))
            .unwrap_or(json!(agtuuid_default.clone()))
            .as_str()
            .unwrap_or(&agtuuid_default)
            .to_string();

        let socket_port = store
            .get("socket_port", Some(json!(8080u16)))
            .unwrap_or(json!(8080u16))
            .as_u64()
            .unwrap_or(8080) as u16;

        Self {
            agtuuid,
            workers:              kv_u32!("workers",              2u32),
            socket_host:          kv_str!("socket_host",          "0.0.0.0"),
            socket_port,
            secret_digest:        kv_str!("secret_digest",        sha256::digest("changeme").as_str()),
            client_control_url:   kv_str!("client_control_url",   "http://localhost:8080"),
            log_level_app:        kv_level!("log_level_app",      "Info"),
            log_level_api:        kv_level!("log_level_api",      "Info"),
            peer_timeout_secs:    kv_u32!("peer_timeout_secs",    60u32),
            peer_refresh_secs:    kv_u32!("peer_refresh_secs",    30u32),
            max_weight:           kv_u32!("max_weight",           600u32),
            ticket_timeout_secs:  kv_u32!("ticket_timeout_secs",  600u32),
            message_timeout_secs: kv_u32!("message_timeout_secs", 600u32),
        }
    }

    /// Decode `secret_digest` (hex string) into a 32-byte AES-256 key.
    pub fn key(&self) -> [u8; 32] {
        let bytes = hex::decode(&self.secret_digest)
            .expect("secret_digest is not valid hex");
        assert!(bytes.len() >= 32, "secret_digest must decode to at least 32 bytes");
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes[..32]);
        key
    }

    /// Log the current configuration values.
    pub fn log(&self) {
        log::info!(
            "\n  agtuuid:              {}\n  workers:              {}\n  socket_host:          {}\n  socket_port:          {}\n  secret_digest:        {}\n  client_control_url:   {}\n  log_level_app:        {}\n  log_level_api:        {}\n  peer_timeout_secs:    {}\n  peer_refresh_secs:    {}\n  max_weight:           {}\n  ticket_timeout_secs:  {}\n  message_timeout_secs: {}",
            self.agtuuid, self.workers, self.socket_host, self.socket_port,
            self.secret_digest, self.client_control_url,
            self.log_level_app, self.log_level_api,
            self.peer_timeout_secs, self.peer_refresh_secs, self.max_weight,
            self.ticket_timeout_secs, self.message_timeout_secs,
        );
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::load()
    }
}
