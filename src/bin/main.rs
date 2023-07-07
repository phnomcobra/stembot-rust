use actix_web::{http::Error, rt::spawn, web, App, HttpRequest, HttpResponse, HttpServer, Result};
use aes::Aes256;
use base64::{engine::general_purpose as b64engine, Engine as _};
use clokwerk::{AsyncScheduler, Interval::Seconds};
use eax::{
    aead::{generic_array::GenericArray, Aead, KeyInit, OsRng},
    AeadInPlace, Eax, Nonce,
};
use futures_util::StreamExt;
use rand::{distributions::Slice, Rng};
use reqwest::{
    header::{HeaderName, HeaderValue},
    Client, StatusCode,
};
use std::{str::FromStr, time::Duration};
use tokio::time::sleep;

use stembot_rust::{config::Configuration, init_logger};

pub type Aes256Eax = Eax<Aes256>;

async fn schedule_test() {
    log::info!("scheduler test");
}

async fn http_request_test() {
    let configuration = Configuration::new_from_cli();

    // Harden this to filter out empty tokens between the host and endpoint
    let url = format!(
        "http://{}:{}{}",
        configuration.host, configuration.port, configuration.endpoint
    );

    let client = Client::new();
    let body = String::from("http request body");

    let key = GenericArray::from_slice(configuration.secret.as_bytes());
    let cipher = Aes256Eax::new(&key);

    let nonce = rand::random::<[u8; 32]>();

    let nonce_array = GenericArray::from_slice(&nonce);
    let mut ciphertext = vec![];
    let tag = cipher
        .encrypt_in_place_detached(nonce_array, body.as_bytes(), &mut ciphertext)
        .expect("encryption failure!");
    let b64_request_body = b64engine::STANDARD.encode(ciphertext);

    match client
        .post(url)
        .body(b64_request_body)
        .header("Nonce", b64engine::STANDARD.encode(nonce))
        .header("Tag", b64engine::STANDARD.encode(tag))
        .send()
        .await
    {
        Ok(response) => {
            log::info!("{:?}", response);

            match response.status() {
                StatusCode::OK => {
                    log::info!("http request test...ok");
                    let header_value = response.headers().get("headerkey").unwrap();
                    log::info!("HEADERKEY: {}", header_value.to_str().unwrap());

                    let response_b64body = response.bytes().await.unwrap();
                    let response_body = b64engine::STANDARD.decode(response_b64body).unwrap();

                    log::info!("{}", String::from_utf8_lossy(&response_body));
                }
                _ => log::error!("http request test...error"),
            }
        }
        Err(error) => {
            log::error!("http request test...error");
            log::error!("{:?}", error);
        }
    }
}

// async fn index(mut payload: web::Payload) -> Result<HttpResponse, Error> {
async fn index(mut payload: web::Payload, request: HttpRequest) -> Result<HttpResponse, Error> {
    let header_key_value = request.headers().get("headerkey").unwrap();
    log::info!("HEADERKEY: {}", header_key_value.to_str().unwrap());

    let mut request_b64body = web::BytesMut::new();

    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;

        request_b64body.extend_from_slice(&chunk);
    }

    let request_body = b64engine::STANDARD.decode(request_b64body).unwrap();
    log::info!("{}", String::from_utf8_lossy(&request_body));

    // Processing message
    let response_string = String::from("response body");
    let response_body = response_string.as_bytes();

    let response_b64body = b64engine::STANDARD.encode(response_body);

    let mut http_response = HttpResponse::Ok().body(response_b64body);

    // Response headers
    let http_response_headers = http_response.headers_mut();
    http_response_headers.append(
        HeaderName::from_str("headerkey").unwrap(),
        HeaderValue::from_str("headervalue").unwrap(),
    );

    Ok(http_response)
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
