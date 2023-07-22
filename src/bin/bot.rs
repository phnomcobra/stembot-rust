use actix_web::{rt::spawn, web, App, HttpServer, Result};
use clokwerk::{AsyncScheduler, Interval::Seconds};
use tracing_actix_web::TracingLogger;

use std::time::Duration;
use tokio::time::sleep;

use stembot_rust::{
    backlog::{poll_backlogs, process_backlog, push_message_collection_to_backlog},
    init_logger,
    io::http::endpoint::message_handler,
    messaging::{Message, MessageCollection, TraceRequest},
    peering::initialize_peers,
    routing::{advertise, age_routes, initialize_routes},
    state::Singleton,
};

async fn test(_singleton: Singleton) {
    // for item in _singleton.peers.read().await.iter() {
    //     log::warn!("{:?}", item);
    // }

    // for item in _singleton.routes.read().await.iter() {
    //     log::warn!("{:?}", item);
    // }

    // log::warn!("backlog length: {}", _singleton.backlog.read().await.len());
    // for item in _singleton.backlog.read().await.iter() {
    //     log::warn!("{:?}", item);
    // }
}

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    let singleton = Singleton::new_from_cli();

    init_logger(singleton.configuration.loglevel.clone());

    log::info!("Starting stembot...");

    log::info!("Initializing peer table...");
    initialize_peers(singleton.clone()).await;

    log::info!("Initializing routing table...");
    initialize_routes(singleton.clone()).await;

    let mut scheduler = AsyncScheduler::new();

    for ping in singleton.configuration.ping.iter() {
        log::info!(
            "Registering ping to \"{}\" every {} seconds...",
            ping.1.destination_id.clone(),
            ping.1.delay.clone()
        );

        scheduler.every(Seconds(ping.1.delay)).run({
            let singleton = singleton.clone();

            let message_collection = MessageCollection {
                origin_id: singleton.configuration.id.clone(),
                destination_id: Some(ping.1.destination_id.clone()),
                messages: vec![Message::Ping],
            };

            move || {
                push_message_collection_to_backlog(message_collection.clone(), singleton.clone())
            }
        });
    }

    for trace in singleton.configuration.trace.iter() {
        log::info!(
            "Registering trace \"{}\" to \"{}\" every {} seconds...",
            trace
                .1
                .request_id
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            trace.1.destination_id.clone(),
            trace.1.delay.clone(),
        );

        scheduler.every(Seconds(trace.1.delay)).run({
            let trace = trace.1.clone();
            let singleton = singleton.clone();

            move || {
                let trace_request_message = match trace.request_id.clone() {
                    Some(id) => Message::TraceRequest(TraceRequest::new(id)),
                    None => Message::TraceRequest(TraceRequest::default()),
                };

                let message_collection = MessageCollection {
                    origin_id: singleton.configuration.id.clone(),
                    destination_id: Some(trace.destination_id.clone()),
                    messages: vec![trace_request_message],
                };

                push_message_collection_to_backlog(message_collection, singleton.clone())
            }
        });
    }

    scheduler.every(Seconds(1)).run({
        let singleton = singleton.clone();
        move || poll_backlogs(singleton.clone())
    });

    scheduler.every(Seconds(1)).run({
        let singleton = singleton.clone();
        move || advertise(singleton.clone())
    });

    scheduler.every(Seconds(1)).run({
        let singleton = singleton.clone();
        move || age_routes(singleton.clone())
    });

    scheduler.every(Seconds(1)).run({
        let singleton = singleton.clone();
        move || test(singleton.clone())
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
        let singleton = singleton.clone();
        async move {
            loop {
                process_backlog(singleton.clone()).await;

                sleep(Duration::from_millis(10)).await;
            }
        }
    });

    log::info!("Starting webserver...");
    // This is bad
    if singleton.configuration.tracing {
        HttpServer::new({
            let singleton = singleton.clone();

            move || {
                App::new()
                    .wrap(TracingLogger::default())
                    .app_data(web::Data::new(singleton.clone()))
                    .route(
                        &singleton.configuration.endpoint,
                        web::post().to(message_handler),
                    )
            }
        })
        .bind((singleton.configuration.host, singleton.configuration.port))?
        .run()
        .await
    } else {
        HttpServer::new({
            let singleton = singleton.clone();

            move || {
                App::new()
                    .app_data(web::Data::new(singleton.clone()))
                    .route(
                        &singleton.configuration.endpoint,
                        web::post().to(message_handler),
                    )
            }
        })
        .bind((singleton.configuration.host, singleton.configuration.port))?
        .run()
        .await
    }
}
