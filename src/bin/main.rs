use actix_web::{http::Error, rt::spawn, web, App, HttpRequest, HttpResponse, HttpServer, Result};
use base64::{engine::general_purpose as b64engine, Engine as _};
use clokwerk::{AsyncScheduler, Interval::Seconds};

use futures_util::StreamExt;
use reqwest::{
    header::{HeaderName, HeaderValue},
    Client, StatusCode,
};
use std::{str::FromStr, time::Duration};
use tokio::time::sleep;

use magic_crypt::{new_magic_crypt, MagicCryptTrait};

use stembot_rust::{config::Configuration, init_logger};

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

    let mut nonce = b64engine::STANDARD.encode(rand::random::<[u8; 32]>());

    let mut cipher = new_magic_crypt!(&configuration.secret, 256, &nonce);

    let mut tag = b64engine::STANDARD.encode(rand::random::<[u8; 32]>());

    let body_as_bytes = body.as_bytes();

    let b64_request_body = cipher.encrypt_bytes_to_base64(body_as_bytes);

    match client
        .post(url)
        .body(b64_request_body)
        .header("Nonce", nonce)
        .header("Tag", tag)
        .send()
        .await
    {
        Ok(response) => {
            log::info!("{:?}", response);

            match response.status() {
                StatusCode::OK => {
                    log::info!("http request test...ok");

                    nonce =
                        String::from(response.headers().get("Nonce").unwrap().to_str().unwrap());
                    tag = String::from(response.headers().get("Tag").unwrap().to_str().unwrap());

                    cipher = new_magic_crypt!(&configuration.secret, 256, &nonce);

                    let response_b64body = response.bytes().await.unwrap();
                    let response_body = cipher
                        .decrypt_base64_to_bytes(
                            String::from_utf8(response_b64body.to_vec()).unwrap(),
                        )
                        .unwrap();

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

async fn index(mut payload: web::Payload, request: HttpRequest) -> Result<HttpResponse, Error> {
    let configuration = Configuration::new_from_cli();

    let mut nonce = request
        .headers()
        .get("Nonce")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let mut tag = request
        .headers()
        .get("Tag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let mut cipher = new_magic_crypt!(&configuration.secret, 256, &nonce);

    let mut request_body_as_bytes = web::BytesMut::new();

    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;

        request_body_as_bytes.extend_from_slice(&chunk);
    }
    let request_body = String::from_utf8(request_body_as_bytes.as_ref().to_vec()).unwrap();

    let request_body = cipher.decrypt_base64_to_bytes(request_body).unwrap();
    log::info!("{}", String::from_utf8_lossy(&request_body));

    nonce = b64engine::STANDARD.encode(rand::random::<[u8; 32]>());
    tag = b64engine::STANDARD.encode(rand::random::<[u8; 32]>());

    cipher = new_magic_crypt!(&configuration.secret, 256, &nonce);

    // Processing message
    let response_string = String::from("response body");
    let response_body = response_string.as_bytes();

    let response_b64body = cipher.encrypt_bytes_to_base64(response_body);

    let mut http_response = HttpResponse::Ok().body(response_b64body);

    // Response headers
    let http_response_headers = http_response.headers_mut();

    http_response_headers.append(
        HeaderName::from_str("Tag").unwrap(),
        HeaderValue::from_str(&tag).unwrap(),
    );

    http_response_headers.append(
        HeaderName::from_str("Nonce").unwrap(),
        HeaderValue::from_str(&nonce).unwrap(),
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
