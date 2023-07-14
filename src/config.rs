use std::collections::HashMap;

use clap::Parser;
use serde::Deserialize;

use toml::de::Error;

use crate::peering::Peer;

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub secret: String,
    pub id: String,
    pub host: String,
    pub port: u16,
    pub endpoint: String,
    pub peer: HashMap<String, Peer>,
    pub maxrouteweight: usize,
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
