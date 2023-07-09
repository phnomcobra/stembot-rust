use actix_web::{rt::spawn, web, App, HttpServer, Result};
use clokwerk::{AsyncScheduler, Interval::Seconds};

use std::time::Duration;
use tokio::time::sleep;

use stembot_rust::{
    config::Configuration,
    init_logger,
    io::http::{client::send_message, endpoint::message_handler},
};

async fn schedule_send_test() {
    send_message().await;
}

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    init_logger();

    log::info!("Starting stembot...");

    let configuration = Configuration::new_from_cli();

    log::info!("{}", format!("{:?}", configuration));

    let mut scheduler = AsyncScheduler::new();

    scheduler.every(Seconds(1)).run(schedule_send_test);

    log::info!("Starting scheduler...");
    spawn(async move {
        loop {
            scheduler.run_pending().await;
            sleep(Duration::from_millis(10)).await;
        }
    });

    log::info!("Starting webserver...");
    HttpServer::new({
        let configuration = configuration.clone();

        move || App::new().route(&configuration.endpoint, web::post().to(message_handler))
    })
    .bind((configuration.host, configuration.port))?
    .run()
    .await
}
