use chrono::{SecondsFormat, Utc};
use log::LevelFilter;
use std::{io::Write, thread};

pub mod config;
pub mod io;
pub mod messaging;
pub mod peering;
pub mod processing;
pub mod routing;

pub fn init_logger() {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
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
