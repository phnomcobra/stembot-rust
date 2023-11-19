use anyhow::Result;
use magic_crypt::{new_magic_crypt, MagicCryptTrait};
use reqwest::Client;
use reqwest::StatusCode;

use crate::config::Configuration;

pub async fn send_raw_message<T: Into<Vec<u8>>, U: Into<String>, V: Into<Configuration>>(
    message: T,
    url: U,
    configuration: V,
) -> Result<Vec<u8>> {
    let configuration = configuration.into();

    let unencrypted_request_body: Vec<u8> = message.into();

    let url = url.into();

    let client = Client::new();

    let nonce_string = sha256::digest(rand::random::<[u8; 32]>().as_ref());

    let cipher = new_magic_crypt!(&configuration.public_http.secret, 256, &nonce_string);

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
                    None => return Err(anyhow::Error::msg("nonce missing from response headers")),
                };

                let nonce_string = match nonce_header_value.to_str() {
                    Ok(value) => String::from(value),
                    Err(_) => {
                        return Err(anyhow::Error::msg(
                            "failed to read nonce from response header",
                        ))
                    }
                };

                let tag_header_value = match response.headers().get("Tag") {
                    Some(value) => value,
                    None => return Err(anyhow::Error::msg("tag missing from response headers")),
                };

                let tag_string = match tag_header_value.to_str() {
                    Ok(value) => String::from(value),
                    Err(_) => {
                        return Err(anyhow::Error::msg(
                            "failed to read tag from response header",
                        ))
                    }
                };

                let encrypted_response_body_bytes = match response.bytes().await {
                    Ok(bytes) => bytes.to_vec(),
                    Err(error) => {
                        return Err(anyhow::Error::msg(format!(
                            "failed to receive encrypted http response body: {error}",
                        )))
                    }
                };

                let encrypted_response_body_string =
                    match String::from_utf8(encrypted_response_body_bytes) {
                        Ok(string) => string,
                        Err(error) => {
                            return Err(anyhow::Error::msg(format!(
                                "failed to read encrypted response body: {error}",
                            )))
                        }
                    };

                let cipher =
                    new_magic_crypt!(&configuration.public_http.secret, 256, &nonce_string);

                let decrypted_response_body =
                    match cipher.decrypt_base64_to_bytes(encrypted_response_body_string) {
                        Ok(bytes) => bytes,
                        Err(error) => {
                            return Err(anyhow::Error::msg(format!(
                                "failed to decrypt http response body: {error}"
                            )))
                        }
                    };

                match tag_string == sha256::digest(decrypted_response_body.as_slice()) {
                    true => Ok(decrypted_response_body),
                    false => Err(anyhow::Error::msg("http response body digest mismatch")),
                }
            }
            _ => Err(anyhow::Error::msg(format!(
                "HTTP Status: {}",
                response.status()
            ))),
        },
        Err(error) => Err(anyhow::Error::msg(format!("HTTP Failure: {}", error))),
    }
}
