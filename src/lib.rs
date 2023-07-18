use chrono::{SecondsFormat, Utc};
use log::Level;
use std::{io::Write, str::FromStr, thread};

pub mod backlog;
pub mod config;
pub mod io;
pub mod messaging;
pub mod peering;
pub mod processing;
pub mod routing;

pub fn init_logger(loglevel: String) {
    env_logger::builder()
        .filter_level(
            Level::from_str(&loglevel)
                .expect("invalid log level filter specified")
                .to_level_filter(),
        )
        .format(move |buf, record| {
            writeln!(
                buf,
                "[{:<5} {} {} {}:{:<3}] {}",
                buf.default_styled_level(record.level()),
                Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
                thread::current().name().unwrap_or("_"),
                record.file().unwrap(),
                record.line().unwrap(),
                record.args()
            )
        })
        .parse_default_env()
        .write_style(env_logger::WriteStyle::Auto)
        .init();
}
