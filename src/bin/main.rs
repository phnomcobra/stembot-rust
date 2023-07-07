use actix_web::{http::Error, rt::spawn, web, App, HttpResponse, HttpServer, Result};
use clokwerk::{AsyncScheduler, Interval::Seconds};
use futures_util::StreamExt;
use std::time::Duration;
use tokio::time::sleep;

use stembot_rust::{config::Configuration, init_logger};

async fn schedule_test() {
    log::info!("scheduler test");
}

async fn index(mut payload: web::Payload) -> Result<HttpResponse, Error> {
    let mut body = web::BytesMut::new();

    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;

        body.extend_from_slice(&chunk);
    }

    Ok(HttpResponse::Ok().body(body))
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

    scheduler.every(Seconds(1)).run(schedule_test);

    spawn(async move {
        loop {
            scheduler.run_pending().await;
            sleep(Duration::from_millis(10)).await;
        }
    });

    log::info!("Starting webserver...");
    HttpServer::new(|| App::new().route("/", web::post().to(index)))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
