use std::error::Error;

use reqwest::{Client, StatusCode};

use magic_crypt::{new_magic_crypt, MagicCryptTrait};

use crate::config::Configuration;

pub async fn send_message<T: Into<Vec<u8>>>(message: T) -> Result<Vec<u8>, Box<dyn Error>> {
    let configuration = Configuration::new_from_cli();

    let unencrypted_request_body: Vec<u8> = message.into();

    // Harden this to filter out empty tokens between the host and endpoint
    let url = format!(
        "http://{}:{}{}",
        configuration.host, configuration.port, configuration.endpoint
    );

    let client = Client::new();

    let nonce_string = sha256::digest(rand::random::<[u8; 32]>().as_ref());

    let cipher = new_magic_crypt!(&configuration.secret, 256, &nonce_string);

    let tag_string = sha256::digest(unencrypted_request_body.as_slice());

    let encrypted_request_body =
        cipher.encrypt_bytes_to_base64(unencrypted_request_body.as_slice());

    match client
        .post(url)
        .body(encrypted_request_body)
        .header("Nonce", nonce_string)
        .header("Tag", tag_string)
        .send()
        .await
    {
        Ok(response) => match response.status() {
            StatusCode::OK => {
                let nonce_header_value = match response.headers().get("Nonce") {
                    Some(value) => value,
                    None => return Err("nonce missing from response headers".into()),
                };

                let nonce_string = match nonce_header_value.to_str() {
                    Ok(value) => String::from(value),
                    Err(_) => return Err("failed to read nonce from response header".into()),
                };

                let tag_header_value = match response.headers().get("Tag") {
                    Some(value) => value,
                    None => return Err("tag missing from response headers".into()),
                };

                let tag_string = match tag_header_value.to_str() {
                    Ok(value) => String::from(value),
                    Err(_) => return Err("failed to read tag from response header".into()),
                };

                let encrypted_response_body_bytes = match response.bytes().await {
                    Ok(bytes) => bytes.to_vec(),
                    Err(_) => return Err("failed to receive encrypted http response body".into()),
                };

                let encrypted_response_body_string =
                    match String::from_utf8(encrypted_response_body_bytes) {
                        Ok(string) => string,
                        Err(_) => return Err("failed to read encrypted response body".into()),
                    };

                let cipher = new_magic_crypt!(&configuration.secret, 256, &nonce_string);

                let decrypted_response_body =
                    match cipher.decrypt_base64_to_bytes(encrypted_response_body_string) {
                        Ok(bytes) => bytes,
                        Err(_) => return Err("failed to decrypt http response body".into()),
                    };

                match tag_string == sha256::digest(decrypted_response_body.as_slice()) {
                    true => Ok(decrypted_response_body),
                    false => return Err("http response body digest mismatch".into()),
                }
            }
            _ => Err(format!("HTTP Status: {}", response.status()).into()),
        },
        Err(error) => Err(format!("HTTP Failure: {}", error).into()),
    }
}
