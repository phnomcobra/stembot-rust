use actix_web::{rt::spawn, web, App, HttpServer, Result};
use clokwerk::{AsyncScheduler, Interval::Seconds};
use tracing_actix_web::TracingLogger;

use std::{sync::Arc, time::Duration};
use tokio::{sync::RwLock, time::sleep};

use stembot_rust::{
    backlog::{poll_backlogs, process_backlog, push_message_collection_to_backlog},
    config::Configuration,
    init_logger,
    io::http::endpoint::message_handler,
    messaging::{Message, MessageCollection},
    peering::{initialize_peers, Peer},
    routing::{advertise, age_routes, initialize_routes, Route},
};

async fn test(
    _peers: Arc<RwLock<Vec<Peer>>>,
    _routes: Arc<RwLock<Vec<Route>>>,
    _backlog: Arc<RwLock<Vec<MessageCollection>>>,
) {
    // for item in _peers.read().await.iter() {
    //     log::warn!("{:?}", item);
    // }

    // for item in _routes.read().await.iter() {
    //     log::warn!("{:?}", item);
    // }

    // log::warn!("backlog length: {}", _backlog.read().await.len());
    // for item in _backlog.read().await.iter() {
    //     log::warn!("{:?}", item);
    // }
}

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    let configuration = Configuration::new_from_cli();

    init_logger(configuration.loglevel.clone());

    let peering_table: Arc<RwLock<Vec<Peer>>> = Arc::new(RwLock::new(vec![]));
    let routing_table: Arc<RwLock<Vec<Route>>> = Arc::new(RwLock::new(vec![]));
    let message_backlog: Arc<RwLock<Vec<MessageCollection>>> = Arc::new(RwLock::new(vec![]));

    log::info!("Starting stembot...");

    log::info!("Initializing peer table...");
    initialize_peers(configuration.clone(), peering_table.clone()).await;

    log::info!("Initializing routing table...");
    initialize_routes(configuration.clone(), routing_table.clone()).await;

    let mut scheduler = AsyncScheduler::new();

    for ping in configuration.ping.iter() {
        log::info!(
            "Registering ping to \"{}\" every {} seconds...",
            ping.1.destination_id.clone(),
            ping.1.delay.clone()
        );

        scheduler.every(Seconds(ping.1.delay)).run({
            let configuration = configuration.clone();
            let message_backlog = message_backlog.clone();
            let message_collection = MessageCollection {
                origin_id: configuration.id.clone(),
                destination_id: Some(ping.1.destination_id.clone()),
                messages: vec![Message::Ping],
            };

            move || {
                push_message_collection_to_backlog(
                    message_collection.clone(),
                    message_backlog.clone(),
                )
            }
        });
    }

    scheduler.every(Seconds(1)).run({
        let configuration = configuration.clone();
        let peering_table = peering_table.clone();
        let message_backlog = message_backlog.clone();
        move || {
            poll_backlogs(
                configuration.clone(),
                peering_table.clone(),
                message_backlog.clone(),
            )
        }
    });

    scheduler.every(Seconds(1)).run({
        let configuration = configuration.clone();
        let peering_table = peering_table.clone();
        let routing_table = routing_table.clone();
        let message_backlog = message_backlog.clone();
        move || {
            advertise(
                configuration.clone(),
                peering_table.clone(),
                routing_table.clone(),
                message_backlog.clone(),
            )
        }
    });

    scheduler.every(Seconds(1)).run({
        let configuration = configuration.clone();
        let routing_table = routing_table.clone();
        move || age_routes(configuration.clone(), routing_table.clone())
    });

    scheduler.every(Seconds(1)).run({
        let peering_table = peering_table.clone();
        let routing_table = routing_table.clone();
        let backlog = message_backlog.clone();
        move || {
            test(
                peering_table.clone(),
                routing_table.clone(),
                backlog.clone(),
            )
        }
    });

    log::info!("Starting scheduler...");
    spawn({
        async move {
            loop {
                scheduler.run_pending().await;
                sleep(Duration::from_millis(10)).await;
            }
        }
    });

    log::info!("Starting backlog...");
    spawn({
        let configuration = configuration.clone();
        let routing_table = routing_table.clone();
        let peering_table = peering_table.clone();
        let message_backlog = message_backlog.clone();

        async move {
            loop {
                process_backlog(
                    configuration.clone(),
                    peering_table.clone(),
                    routing_table.clone(),
                    message_backlog.clone(),
                )
                .await;

                sleep(Duration::from_millis(10)).await;
            }
        }
    });

    log::info!("Starting webserver...");
    // This is bad
    if configuration.tracing {
        HttpServer::new({
            let configuration = configuration.clone();

            move || {
                App::new()
                    .wrap(TracingLogger::default())
                    .app_data(web::Data::new(configuration.clone()))
                    .app_data(web::Data::new(peering_table.clone()))
                    .app_data(web::Data::new(routing_table.clone()))
                    .app_data(web::Data::new(message_backlog.clone()))
                    .route(&configuration.endpoint, web::post().to(message_handler))
            }
        })
        .bind((configuration.host, configuration.port))?
        .run()
        .await
    } else {
        HttpServer::new({
            let configuration = configuration.clone();

            move || {
                App::new()
                    .app_data(web::Data::new(configuration.clone()))
                    .app_data(web::Data::new(peering_table.clone()))
                    .app_data(web::Data::new(routing_table.clone()))
                    .app_data(web::Data::new(message_backlog.clone()))
                    .route(&configuration.endpoint, web::post().to(message_handler))
            }
        })
        .bind((configuration.host, configuration.port))?
        .run()
        .await
    }
}
