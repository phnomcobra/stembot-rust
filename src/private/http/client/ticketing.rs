use reqwest::Client;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::core::ticket::TicketMessage;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Session {
    ticket_message: Option<TicketMessage>,
    ticket_id: Option<String>,
    destination_id: Option<String>,
}

pub async fn request_ticket_initialization(
    ticket_message: TicketMessage,
    ticket_id: Option<String>,
    destination_id: Option<String>,
    url: String,
) -> anyhow::Result<Option<String>> {
    let client = Client::new();

    let session = Session {
        ticket_message: Some(ticket_message),
        ticket_id,
        destination_id,
    };

    let body = serde_json::to_string_pretty(&session)?.as_bytes().to_vec();

    match client.post(url.clone()).body(body).send().await {
        Ok(response) => match response.status() {
            StatusCode::OK => {
                let response_body_bytes = response.bytes().await?.to_vec();

                match serde_json::from_slice::<Session>(&response_body_bytes) {
                    Ok(session) => Ok(session.ticket_id),
                    Err(error) => Err(anyhow::Error::msg(format!(
                        "failed to parse session body: {error}"
                    ))),
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

pub async fn request_ticket_retrieval(
    ticket_id: Option<String>,
    url: String,
) -> anyhow::Result<TicketMessage> {
    let client = Client::new();

    let session = Session {
        ticket_message: None,
        ticket_id,
        destination_id: None,
    };

    let body = serde_json::to_string_pretty(&session)?.as_bytes().to_vec();

    match client.get(url.clone()).body(body).send().await {
        Ok(response) => match response.status() {
            StatusCode::OK => {
                let response_body_bytes = response.bytes().await?.to_vec();

                match serde_json::from_slice::<Session>(&response_body_bytes) {
                    Ok(session) => match session.ticket_message {
                        Some(ticket_message) => Ok(ticket_message),
                        None => Err(anyhow::Error::msg("ticket not set in session body")),
                    },
                    Err(error) => Err(anyhow::Error::msg(format!(
                        "failed to parse session body: {error}"
                    ))),
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

pub async fn request_ticket_synchronization(
    ticket_message: TicketMessage,
    ticket_id: Option<String>,
    destination_id: Option<String>,
    url: String,
) -> anyhow::Result<TicketMessage> {
    let client = Client::new();

    let session = Session {
        ticket_message: Some(ticket_message),
        ticket_id,
        destination_id,
    };

    let body = serde_json::to_string_pretty(&session)?.as_bytes().to_vec();

    match client.post(url.clone()).body(body).send().await {
        Ok(response) => match response.status() {
            StatusCode::OK => {
                let response_body_bytes = response.bytes().await?.to_vec();

                match serde_json::from_slice::<Session>(&response_body_bytes) {
                    Ok(session) => match session.ticket_message {
                        Some(ticket_message) => Ok(ticket_message),
                        None => Err(anyhow::Error::msg("ticket not set in session body")),
                    },
                    Err(error) => Err(anyhow::Error::msg(format!(
                        "failed to parse session body: {error}"
                    ))),
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
