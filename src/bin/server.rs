use actix_web::{rt::spawn, web, App, HttpServer, Result};
use clokwerk::{AsyncScheduler, Interval::Seconds};
use tracing_actix_web::TracingLogger;

use std::time::Duration;
use tokio::time::sleep;

use stembot_rust::{
    config::config, logger::init_logger, messaging::expire_network_messages, processor::test_handler, ticketing::expire_tickets
};

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    let config = config();


    init_logger(config.log_level_app.to_string());

    config.log();


    let mut scheduler = AsyncScheduler::new();

    scheduler.every(Seconds(config.message_timeout_secs)).run({
        move || async move {
            expire_tickets().unwrap_or_else(|e| log::error!("Error expiring tickets: {e}"));
        }
    });

    scheduler.every(Seconds(config.ticket_timeout_secs)).run({
        move || async move {
            expire_network_messages().unwrap_or_else(|e| log::error!("Error expiring network messages: {e}"));
        }
    });

    log::info!("Starting scheduler");

    spawn({
        async move {
            loop {
                scheduler.run_pending().await;
                sleep(Duration::from_secs(1)).await;
            }
        }
    });

    let server = HttpServer::new(
        move || {
            App::new()
                .wrap(TracingLogger::default())
                .app_data(web::Data::new(config.clone()))
                .route("/test", web::post().to(test_handler),)
        }
    );

    log::info!("Starting server");

    server.bind(
        (
            config.socket_host.clone(),
            config.socket_port,
        )
    )?.run().await
}
