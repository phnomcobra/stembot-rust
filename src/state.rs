use crate::models::config::Config;

#[derive(Clone, Debug)]
pub struct Singleton {
    pub config: Config,
}

impl Default for Singleton {
    fn default() -> Self {
        Self { config: Config::load() }
    }
}
