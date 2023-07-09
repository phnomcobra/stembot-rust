use actix_web::{http::Error, web, HttpRequest, HttpResponse, Result};
use futures_util::StreamExt;
use reqwest::header::{HeaderName, HeaderValue};
use std::str::FromStr;

use magic_crypt::{new_magic_crypt, MagicCryptTrait};

use crate::config::Configuration;

pub async fn message_handler(
    mut payload: web::Payload,
    request: HttpRequest,
) -> Result<HttpResponse, Error> {
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

    assert_eq!(tag, sha256::digest(request_body.as_slice()));

    log::info!("{}", String::from_utf8_lossy(&request_body));

    nonce = sha256::digest(rand::random::<[u8; 32]>().as_ref());

    cipher = new_magic_crypt!(&configuration.secret, 256, &nonce);

    // Processing message
    let response_string = String::from("response body");
    let response_body = response_string.as_bytes();

    tag = sha256::digest(response_body);

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
