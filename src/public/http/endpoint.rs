use actix_web::{web, HttpRequest, HttpResponse, Result};
use reqwest::header::{HeaderName, HeaderValue};
use std::error::Error;

use magic_crypt::{new_magic_crypt, MagicCryptTrait};

use crate::core::{
    messaging::MessageCollection, processing::process_message_collection, state::Singleton,
};

pub async fn message_handler(
    encrypted_request_body_bytes: web::Bytes,
    singleton: web::Data<Singleton>,
    request: HttpRequest,
) -> Result<HttpResponse, Box<dyn Error>> {
    let singleton = singleton.get_ref();

    let request_nonce_header_value: &HeaderValue = match request.headers().get("Nonce") {
        Some(value) => value,
        None => return Err("nonce missing from request headers".into()),
    };

    let request_nonce_string = match request_nonce_header_value.to_str() {
        Ok(value) => String::from(value),
        Err(_) => return Err("failed to read nonce from request header".into()),
    };

    let request_tag_header_value = match request.headers().get("Tag") {
        Some(value) => value,
        None => return Err("tag missing from request headers".into()),
    };

    let request_tag_string = match request_tag_header_value.to_str() {
        Ok(value) => String::from(value),
        Err(_) => return Err("failed to read tag from request header".into()),
    };

    let request_cipher = new_magic_crypt!(
        &singleton.configuration.public_http.secret,
        256,
        &request_nonce_string
    );

    let encrypted_request_body_string =
        match String::from_utf8(encrypted_request_body_bytes.to_vec()) {
            Ok(bytes) => bytes,
            Err(_) => return Err("failed to read encrypted request body".into()),
        };

    let decrypted_request_body =
        match request_cipher.decrypt_base64_to_bytes(encrypted_request_body_string) {
            Ok(bytes) => bytes,
            Err(_) => return Err("failed to decrypt http request body".into()),
        };

    let decrypted_response_body_bytes: Vec<u8> =
        match request_tag_string == sha256::digest(decrypted_request_body.as_slice()) {
            true => {
                let inbound_message_collection =
                    match MessageCollection::try_from(decrypted_request_body) {
                        Ok(collection) => collection,
                        Err(error) => return Err(error),
                    };

                match process_message_collection(inbound_message_collection, singleton.clone())
                    .await
                    .try_into()
                {
                    Ok(bytes) => bytes,
                    Err(error) => return Err(error),
                }
            }
            false => return Err("http request body digest mismatch".into()),
        };

    let response_nonce_string = sha256::digest(rand::random::<[u8; 32]>().as_ref());

    let response_nonce_header = match HeaderValue::from_str(&response_nonce_string) {
        Ok(header) => header,
        Err(_) => return Err("failed to instantiate response header value for nonce".into()),
    };

    let response_cipher = new_magic_crypt!(
        &singleton.configuration.public_http.secret,
        256,
        &response_nonce_string
    );

    let response_tag_string = sha256::digest(decrypted_response_body_bytes.as_slice());

    let response_tag_header = match HeaderValue::from_str(&response_tag_string) {
        Ok(header) => header,
        Err(_) => return Err("failed to instantiate response header value for tag".into()),
    };

    let encrypted_response_body_string =
        response_cipher.encrypt_bytes_to_base64(&decrypted_response_body_bytes);

    let mut http_response = HttpResponse::Ok().body(encrypted_response_body_string);

    let http_response_headers = http_response.headers_mut();

    http_response_headers.append(HeaderName::from_static("tag"), response_tag_header);

    http_response_headers.append(HeaderName::from_static("nonce"), response_nonce_header);

    Ok(http_response)
}
