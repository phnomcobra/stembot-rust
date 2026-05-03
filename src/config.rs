use std::sync::OnceLock;

use crate::models::config::Config;

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Return a reference to the process-wide [`Config`] singleton.
///
/// The first call loads the config from the kvstore; every subsequent call
/// returns the cached value without touching the database.
pub fn config() -> &'static Config {
    CONFIG.get_or_init(Config::load)
}
