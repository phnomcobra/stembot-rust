use actix_web::{http::Error, rt::spawn, web, App, HttpResponse, HttpServer, Result};
use clokwerk::{AsyncScheduler, Interval::Seconds};
use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use std::time::Duration;
use tokio::time::sleep;

use stembot_rust::{config::Configuration, init_logger};

async fn schedule_test() {
    log::info!("scheduler test");
}

async fn http_request_test() {
    let configuration = Configuration::new_from_cli();

    let url = format!(
        "http://{}:{}{}",
        configuration.host, configuration.port, configuration.endpoint
    );

    let client = Client::new();

    match client
        .post(url)
        .body("request test")
        .header("headerkey", "headervalue")
        .send()
        .await
    {
        Ok(response) => {
            log::info!("{:?}", response);
            
            match response.status() {
                StatusCode::OK => {
                    log::info!("http request test...ok");
                    let bytes = response.bytes().await.unwrap();
                    log::info!("{}", String::from_utf8_lossy(&bytes))
                },
                _ => log::error!("http request test...error"),
            }
        }
        Err(error) => {
            log::error!("http request test...error");
            log::error!("{:?}", error);
        }
    }
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
    scheduler.every(Seconds(1)).run(http_request_test);

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

        move || App::new().route(&configuration.endpoint, web::post().to(index))
    })
    .bind((configuration.host, configuration.port))?
    .run()
    .await
}
