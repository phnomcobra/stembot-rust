use actix_web::{rt::spawn, web, App, HttpServer, Result};
use clokwerk::{AsyncScheduler, Interval::Seconds};
use tracing_actix_web::TracingLogger;

use std::time::Duration;
use tokio::time::sleep;

use stembot_rust::{
    config::Configuration,
    init_logger,
    io::http::{client::send_message, endpoint::message_handler},
    message::{Message, MessageCollection},
};

async fn advertise(configuration: Configuration) {
    for peer in configuration.clone().peer.into_values() {
        let configuration = configuration.clone();

        let message = Message::Ping;
        let message_collection = MessageCollection {
            messages: vec![message],
            origin_id: configuration.id.clone(),
        };

        match send_message(
            bincode::serialize(&message_collection).unwrap(),
            peer.url,
            configuration,
        )
        .await
        {
            Ok(bytes) => {
                let message_collection: MessageCollection = bincode::deserialize(&bytes).unwrap();
                log::info!("{:?}", message_collection);
            }
            Err(error) => log::error!("{}", error),
        };
    }
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

    scheduler.every(Seconds(1)).run({
        let configuration = configuration.clone();
        move || advertise(configuration.clone())
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
                .route(&configuration.endpoint, web::post().to(message_handler))
        }
    })
    .bind((configuration.host, configuration.port))?
    .run()
    .await
}
