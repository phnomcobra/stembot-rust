use std::error::Error;

use reqwest::{Client, StatusCode};

use magic_crypt::{new_magic_crypt, MagicCryptTrait};

use crate::config::Configuration;

pub async fn send_message<T: Into<Vec<u8>>>(message: T) -> Result<Vec<u8>, Box<dyn Error>> {
    let configuration = Configuration::new_from_cli();

    let body_as_bytes: Vec<u8> = message.into();

    // Harden this to filter out empty tokens between the host and endpoint
    let url = format!(
        "http://{}:{}{}",
        configuration.host, configuration.port, configuration.endpoint
    );

    let client = Client::new();

    let mut nonce = sha256::digest(rand::random::<[u8; 32]>().as_ref());

    let mut cipher = new_magic_crypt!(&configuration.secret, 256, &nonce);

    let mut tag = sha256::digest(body_as_bytes.as_slice());

    let b64_request_body = cipher.encrypt_bytes_to_base64(body_as_bytes.as_slice());

    match client
        .post(url)
        .body(b64_request_body)
        .header("Nonce", nonce)
        .header("Tag", tag)
        .send()
        .await
    {
        Ok(response) => match response.status() {
            StatusCode::OK => {
                nonce = String::from(response.headers().get("Nonce").unwrap().to_str().unwrap());
                tag = String::from(response.headers().get("Tag").unwrap().to_str().unwrap());

                cipher = new_magic_crypt!(&configuration.secret, 256, &nonce);

                let response_b64body = response.bytes().await.unwrap();
                let response_body = cipher
                    .decrypt_base64_to_bytes(String::from_utf8(response_b64body.to_vec()).unwrap())
                    .unwrap();

                assert_eq!(tag, sha256::digest(response_body.as_slice()));

                Ok(response_body)
            }
            _ => {
                let error_str = format!("HTTP Status: {}", response.status());
                log::error!("{}", &error_str);
                Err(error_str.into())
            }
        },
        Err(error) => {
            let error_str = format!("HTTP Failure: {}", error);
            log::error!("{}", &error_str);
            Err(error_str.into())
        }
    }
}
