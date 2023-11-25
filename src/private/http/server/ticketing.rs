use actix_web::{web, HttpResponse};
use std::error::Error;

use crate::core::ticket::exchange::receive_ticket_message;
use crate::core::ticket::exchange::send_ticket_message;
use crate::core::{state::Singleton, ticket::exchange::send_and_receive_ticket_message};
use crate::private::http::Session;

pub async fn ticket_synchronization_endpoint(
    request_body_bytes: web::Bytes,
    singleton: web::Data<Singleton>,
) -> Result<HttpResponse, Box<dyn Error>> {
    let singleton = singleton.get_ref();

    let mut session = match serde_json::from_slice::<Session>(&request_body_bytes) {
        Ok(session) => session,
        Err(_) => return Err("failed to parse session body".into()),
    };

    let ticket_message = match session.ticket_message {
        Some(ticket_message) => ticket_message,
        None => return Err("ticket not set in session body".into()),
    };

    match send_and_receive_ticket_message(
        ticket_message,
        session.ticket_id.clone(),
        session.destination_id.clone(),
        singleton.clone(),
    )
    .await
    {
        Ok(ticket_message) => session.ticket_message = Some(ticket_message),
        Err(error) => return Err(error.into()),
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

    let ticket = match session.ticket_message.clone() {
        Some(ticket) => ticket,
        None => return Err("ticket not set in session body".into()),
    };

    let ticket_id = send_ticket_message(
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

    match receive_ticket_message(ticket_id, singleton.clone()).await {
        Ok(ticket_message) => session.ticket_message = Some(ticket_message),
        Err(error) => return Err(error.to_string().into()),
    };

    let response_body = match serde_json::to_string_pretty(&session) {
        Ok(json_string) => json_string.as_bytes().to_vec(),
        Err(error) => return Err(error.into()),
    };

    Ok(HttpResponse::Ok().body(response_body))
}
