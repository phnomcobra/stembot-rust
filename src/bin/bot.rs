use actix_web::{rt::spawn, web, App, HttpServer, Result};
use clokwerk::{AsyncScheduler, Interval::Seconds};
use tracing_actix_web::TracingLogger;

use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::time::sleep;

use stembot_rust::{
    config::Configuration,
    init_logger,
    io::http::endpoint::message_handler,
    routing::{advertise, initialize_peers, Peer, Route},
};

async fn test(peering_table: Arc<RwLock<Vec<Peer>>>) {
    log::warn!("{:?}", peering_table.read().unwrap());
}

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    init_logger();

    let peering_table: Arc<RwLock<Vec<Peer>>> = Arc::new(RwLock::new(vec![]));
    let routing_table: Arc<RwLock<Vec<Route>>> = Arc::new(RwLock::new(vec![]));

    log::info!("Starting stembot...");

    let configuration = Configuration::new_from_cli();

    log::info!("Initializing peer table...");
    initialize_peers(configuration.clone(), peering_table.clone());

    let mut scheduler = AsyncScheduler::new();

    scheduler.every(Seconds(1)).run({
        let configuration = configuration.clone();
        let peering_table = peering_table.clone();
        let routing_table = routing_table.clone();
        move || {
            advertise(
                configuration.clone(),
                peering_table.clone(),
                routing_table.clone(),
            )
        }
    });

    scheduler.every(Seconds(1)).run({
        let peering_table = peering_table.clone();
        move || test(peering_table.clone())
    });

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

        move || {
            App::new()
                .wrap(TracingLogger::default())
                .app_data(web::Data::new(configuration.clone()))
                .app_data(web::Data::new(peering_table.clone()))
                .app_data(web::Data::new(routing_table.clone()))
                .route(&configuration.endpoint, web::post().to(message_handler))
        }
    })
    .bind((configuration.host, configuration.port))?
    .run()
    .await
}
