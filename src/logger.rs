use chrono::Utc;
use log::Level;
use std::{io::Write, path::Path, str::FromStr};

pub fn init_logger(app_loglevel: String, api_loglevel: String) {
    let api_filter = Level::from_str(&api_loglevel)
        .expect("invalid API log level filter specified")
        .to_level_filter();

    env_logger::builder()
        .filter_level(
            Level::from_str(&app_loglevel)
                .expect("invalid log level filter specified")
                .to_level_filter(),
        )
        .filter_module("actix_web", api_filter)
        .filter_module("actix_server", api_filter)
        .filter_module("actix_http", api_filter)
        .filter_module("tracing_actix_web", api_filter)
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
