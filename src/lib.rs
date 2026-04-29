use chrono::Utc;
use log::Level;
use std::{io::Write, path::Path, str::FromStr};

pub mod executor;
pub mod models;
pub mod processor;
pub mod state;

pub fn init_logger(loglevel: String) {
    env_logger::builder()
        .filter_level(
            Level::from_str(&loglevel)
                .expect("invalid log level filter specified")
                .to_level_filter(),
        )
        .format(move |buf, record| {
            let short_filename = record
                .file()
                .and_then(|f| Path::new(f).file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            writeln!(
                buf,
                "{}|{}|{}:L{}|{}|{}",
                record.level(),
                Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                short_filename,
                record.line().unwrap_or(0),
                record.target(),
                record.args()
            )
        })
        .parse_default_env()
        .init();
}
