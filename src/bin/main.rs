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
    io::http::{client::send_message, endpoint::message_handler},
    message::{Message, MessageCollection},
    routing::{Peer, Route},
};

async fn advertise(configuration: Configuration, peering_table: Arc<RwLock<Vec<Peer>>>) {
    let mut local_peering_table = peering_table.read().unwrap().clone();

    for peer in local_peering_table.iter_mut().filter(|x| x.url.is_some()) {
        let configuration = configuration.clone();

        let message = Message::Ping;
        let message_collection = MessageCollection {
            messages: vec![message],
            origin_id: configuration.id.clone(),
        };

        match send_message(
            bincode::serialize(&message_collection).unwrap(),
            peer.url.as_ref().unwrap(),
            configuration,
        )
        .await
        {
            Ok(bytes) => {
                let message_collection: MessageCollection = bincode::deserialize(&bytes).unwrap();
                peer.id = Some(message_collection.origin_id.clone());

                log::info!("{:?}", message_collection);
            }
            Err(error) => log::error!("{}", error),
        };
    }

    let mut shared_peering_table = peering_table.write().unwrap();
    shared_peering_table.clear();
    shared_peering_table.append(&mut local_peering_table);
}

async fn test(peering_table: Arc<RwLock<Vec<Peer>>>) {
    log::warn!("{:?}", peering_table.read().unwrap());
}

fn initialize_peers(configuration: Configuration, peering_table: Arc<RwLock<Vec<Peer>>>) {
    let mut peering_table = peering_table.write().unwrap();
    for peer in configuration.clone().peer.into_values() {
        peering_table.push(Peer {
            id: None,
            url: peer.url.clone(),
            polling: peer.polling,
        })
    }
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
        move || advertise(configuration.clone(), peering_table.clone())
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
