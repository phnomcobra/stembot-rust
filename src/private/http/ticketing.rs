use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::error::Error;

use reqwest::Client;
use reqwest::StatusCode;

use crate::core::ticketing::receive_ticket;
use crate::core::ticketing::send_ticket;
use crate::core::{messaging::Ticket, state::Singleton, ticketing::send_and_receive_ticket};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Session {
    ticket: Option<Ticket>,
    ticket_id: Option<String>,
    destination_id: Option<String>,
}

pub async fn ticket_synchronization_endpoint(
    request_body_bytes: web::Bytes,
    singleton: web::Data<Singleton>,
) -> Result<HttpResponse, Box<dyn Error>> {
    let singleton = singleton.get_ref();

    let mut session = match serde_json::from_slice::<Session>(&request_body_bytes) {
        Ok(session) => session,
        Err(_) => return Err("failed to parse session body".into()),
    };

    let ticket = match session.ticket {
        Some(ticket) => ticket.clone(),
        None => return Err("ticket not set in session body".into()),
    };

    match send_and_receive_ticket(
        ticket,
        session.ticket_id.clone(),
        session.destination_id.clone(),
        singleton.clone(),
    )
    .await
    {
        Ok(ticket) => session.ticket = Some(ticket),
        Err(error) => return Err(error),
    };

    let response_body = match serde_json::to_string_pretty(&session) {
        Ok(json_string) => json_string.as_bytes().to_vec(),
        Err(error) => return Err(error.into()),
    };

    Ok(HttpResponse::Ok().body(response_body))
}

pub async fn ticket_initialization_endpoint(
    request_body_bytes: web::Bytes,
    singleton: web::Data<Singleton>,
) -> Result<HttpResponse, Box<dyn Error>> {
    let singleton = singleton.get_ref();

    let mut session = match serde_json::from_slice::<Session>(&request_body_bytes) {
        Ok(session) => session,
        Err(_) => return Err("failed to parse session body".into()),
    };

    let ticket = match session.ticket.clone() {
        Some(ticket) => ticket,
        None => return Err("ticket not set in session body".into()),
    };

    let ticket_id = send_ticket(
        ticket,
        session.ticket_id.clone(),
        session.destination_id.clone(),
        singleton.clone(),
    )
    .await;

    session.ticket_id = Some(ticket_id);

    let response_body = match serde_json::to_string_pretty(&session) {
        Ok(json_string) => json_string.as_bytes().to_vec(),
        Err(error) => return Err(error.into()),
    };

    Ok(HttpResponse::Ok().body(response_body))
}

pub async fn ticket_retrieval_endpoint(
    request_body_bytes: web::Bytes,
    singleton: web::Data<Singleton>,
) -> Result<HttpResponse, Box<dyn Error>> {
    let singleton = singleton.get_ref();

    let mut session = match serde_json::from_slice::<Session>(&request_body_bytes) {
        Ok(session) => session,
        Err(_) => return Err("failed to parse session body".into()),
    };

    let ticket_id = match session.ticket_id.clone() {
        Some(ticket_id) => ticket_id,
        None => return Err("ticket id not set in session".into()),
    };

    match receive_ticket(ticket_id, singleton.clone()).await {
        Ok(ticket) => session.ticket = Some(ticket),
        Err(error) => return Err(error.to_string().into()),
    };

    let response_body = match serde_json::to_string_pretty(&session) {
        Ok(json_string) => json_string.as_bytes().to_vec(),
        Err(error) => return Err(error.into()),
    };

    Ok(HttpResponse::Ok().body(response_body))
}

pub async fn request_ticket_initialization(
    ticket: Ticket,
    ticket_id: Option<String>,
    destination_id: Option<String>,
    url: String,
) -> Result<Option<String>, Box<dyn Error + Send + Sync + 'static>> {
    let client = Client::new();

    let session = Session {
        ticket: Some(ticket),
        ticket_id,
        destination_id,
    };

    let body = match serde_json::to_string_pretty(&session) {
        Ok(json_string) => json_string.as_bytes().to_vec(),
        Err(error) => return Err(error.into()),
    };

    match client.post(url.clone()).body(body).send().await {
        Ok(response) => match response.status() {
            StatusCode::OK => {
                let response_body_bytes = match response.bytes().await {
                    Ok(bytes) => bytes.to_vec(),
                    Err(_) => return Err("failed to receive session body".into()),
                };

                match serde_json::from_slice::<Session>(&response_body_bytes) {
                    Ok(session) => Ok(session.ticket_id),
                    Err(_) => Err("failed to parse session body".into()),
                }
            }
            _ => Err(format!("HTTP Status: {}", response.status()).into()),
        },
        Err(error) => Err(format!("HTTP Failure: {}", error).into()),
    }
}

pub async fn request_ticket_retrieval(
    ticket_id: Option<String>,
    url: String,
) -> Result<Ticket, Box<dyn Error + Send + Sync + 'static>> {
    let client = Client::new();

    let session = Session {
        ticket: None,
        ticket_id,
        destination_id: None,
    };

    let body = match serde_json::to_string_pretty(&session) {
        Ok(json_string) => json_string.as_bytes().to_vec(),
        Err(error) => return Err(error.into()),
    };

    match client.get(url.clone()).body(body).send().await {
        Ok(response) => match response.status() {
            StatusCode::OK => {
                let response_body_bytes = match response.bytes().await {
                    Ok(bytes) => bytes.to_vec(),
                    Err(_) => return Err("failed to receive session body".into()),
                };

                match serde_json::from_slice::<Session>(&response_body_bytes) {
                    Ok(session) => match session.ticket.clone() {
                        Some(ticket) => Ok(ticket),
                        None => return Err("ticket not set in session body".into()),
                    },
                    Err(_) => Err("failed to parse session body".into()),
                }
            }
            _ => Err(format!("HTTP Status: {}", response.status()).into()),
        },
        Err(error) => Err(format!("HTTP Failure: {}", error).into()),
    }
}

pub async fn request_ticket_synchronization(
    ticket: Ticket,
    ticket_id: Option<String>,
    destination_id: Option<String>,
    url: String,
) -> Result<Ticket, Box<dyn Error + Send + Sync + 'static>> {
    let client = Client::new();

    let session = Session {
        ticket: Some(ticket),
        ticket_id,
        destination_id,
    };

    let body = match serde_json::to_string_pretty(&session) {
        Ok(json_string) => json_string.as_bytes().to_vec(),
        Err(error) => return Err(error.into()),
    };

    match client.post(url.clone()).body(body).send().await {
        Ok(response) => match response.status() {
            StatusCode::OK => {
                let response_body_bytes = match response.bytes().await {
                    Ok(bytes) => bytes.to_vec(),
                    Err(_) => return Err("failed to receive session body".into()),
                };

                match serde_json::from_slice::<Session>(&response_body_bytes) {
                    Ok(session) => match session.ticket.clone() {
                        Some(ticket) => Ok(ticket),
                        None => return Err("ticket not set in session body".into()),
                    },
                    Err(_) => Err("failed to parse session body".into()),
                }
            }
            _ => Err(format!("HTTP Status: {}", response.status()).into()),
        },
        Err(error) => Err(format!("HTTP Failure: {}", error).into()),
    }
}
