use clap::Parser;
use serde::Deserialize;
use std::collections::HashMap;
use toml::de::Error;

use crate::core::peering::Peer;

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub id: String,
    #[serde(default)]
    pub peer: HashMap<String, Peer>,
    #[serde(default)]
    pub ping: HashMap<String, Ping>,
    #[serde(default)]
    pub trace: HashMap<String, Trace>,
    pub maxrouteweight: usize,
    pub loglevel: String,
    pub ticketexpiration: u64,
    pub public_http: PublicHttp,
    pub private_http: PrivateHttp,
    pub broadcastexpiration: u64,
    pub backlog_period: u64,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicHttp {
    pub secret: String,
    pub tracing: bool,
    pub host: String,
    pub port: u16,
    pub endpoint: String,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateHttp {
    pub tracing: bool,
    pub host: String,
    pub port: u16,
    pub ticket_sync_endpoint: String,
    pub ticket_async_endpoint: String,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ping {
    pub destination_id: String,
    pub delay: u32,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TicketTest {
    pub destination_id: String,
    pub delay: u32,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize)]
pub struct Trace {
    pub destination_id: String,
    pub delay: u32,
    pub request_id: Option<String>,
}

#[derive(Parser, Clone, Debug, Deserialize)]
#[clap(name = "Stembot")]
pub struct CommandLineArguments {
    #[clap(long)]
    config_path: String,
}

impl Configuration {
    /// Load [`Configuration`] arguments from a provided file path,
    pub fn new_from_cli() -> Self {
        let cli_args = CommandLineArguments::parse();
        Self::new_from_file(&cli_args.config_path)
    }

    /// Load [`Configuration`] arguments from a provided file path.
    pub fn new_from_file(config_path: &str) -> Self {
        let file_data = std::fs::read_to_string(config_path);
        let file_data = match file_data {
            Ok(data) => data,
            Err(e) => panic!("ERROR: reading configuration file: {}", e),
        };

        let result: Result<Self, Error> = toml::from_str(&file_data);
        match result {
            Ok(configuration) => configuration,
            Err(e) => panic!("ERROR: reading configuration file: {}", e),
        }
    }
}
