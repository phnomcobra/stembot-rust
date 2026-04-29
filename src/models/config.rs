// use clap::Parser;
use serde::Deserialize;
// use std::collections::HashMap;
// use toml::de::Error;

fn default_id() -> String { String::new() }
fn default_port() -> u16 { 8080 }
fn default_host() -> String { String::from("0.0.0.0") }

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    #[serde(default = "default_id")]
    pub id: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_host")]
    pub host: String,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            id: default_id(),
            port: default_port(),
            host: default_host(),
        }
    }
}
