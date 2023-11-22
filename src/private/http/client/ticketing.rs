use reqwest::Client;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::core::ticket::Ticket;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Session {
    ticket: Option<Ticket>,
    ticket_id: Option<String>,
    destination_id: Option<String>,
}

pub async fn request_ticket_initialization(
    ticket: Ticket,
    ticket_id: Option<String>,
    destination_id: Option<String>,
    url: String,
) -> anyhow::Result<Option<String>> {
    let client = Client::new();

    let session = Session {
        ticket: Some(ticket),
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
) -> anyhow::Result<Ticket> {
    let client = Client::new();

    let session = Session {
        ticket: None,
        ticket_id,
        destination_id: None,
    };

    let body = serde_json::to_string_pretty(&session)?.as_bytes().to_vec();

    match client.get(url.clone()).body(body).send().await {
        Ok(response) => match response.status() {
            StatusCode::OK => {
                let response_body_bytes = response.bytes().await?.to_vec();

                match serde_json::from_slice::<Session>(&response_body_bytes) {
                    Ok(session) => match session.ticket {
                        Some(ticket) => Ok(ticket),
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
    ticket: Ticket,
    ticket_id: Option<String>,
    destination_id: Option<String>,
    url: String,
) -> anyhow::Result<Ticket> {
    let client = Client::new();

    let session = Session {
        ticket: Some(ticket),
        ticket_id,
        destination_id,
    };

    let body = serde_json::to_string_pretty(&session)?.as_bytes().to_vec();

    match client.post(url.clone()).body(body).send().await {
        Ok(response) => match response.status() {
            StatusCode::OK => {
                let response_body_bytes = response.bytes().await?.to_vec();

                match serde_json::from_slice::<Session>(&response_body_bytes) {
                    Ok(session) => match session.ticket {
                        Some(ticket) => Ok(ticket),
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
