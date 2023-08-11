use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::error::Error;

use reqwest::Client;
use reqwest::StatusCode;

use crate::{messaging::Ticket, state::Singleton, ticketing::synchronize_ticket};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketHttpRequest {
    pub ticket: Ticket,
    pub ticket_id: Option<String>,
    pub destination_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketHttpResponse {
    pub ticket: Ticket,
}

pub async fn ticket_synchronization_endpoint(
    request_body_bytes: web::Bytes,
    singleton: web::Data<Singleton>,
) -> Result<HttpResponse, Box<dyn Error>> {
    let singleton = singleton.get_ref();

    let request_body_string = match String::from_utf8(request_body_bytes.to_vec()) {
        Ok(json_string) => json_string,
        Err(_) => return Err("failed to read ticket request body".into()),
    };

    let ticket_http_request = match serde_json::from_str::<TicketHttpRequest>(&request_body_string)
    {
        Ok(ticket_http_request) => ticket_http_request,
        Err(_) => return Err("failed to parse ticket request body".into()),
    };

    let ticket_http_response = match synchronize_ticket(
        ticket_http_request.ticket.clone(),
        ticket_http_request.ticket_id.clone(),
        ticket_http_request.destination_id.clone(),
        singleton.clone(),
    )
    .await
    {
        Ok(ticket) => TicketHttpResponse { ticket },
        Err(error) => return Err(error),
    };

    let response_body = match serde_json::to_string_pretty(&ticket_http_response) {
        Ok(json_string) => json_string.as_bytes().to_vec(),
        Err(error) => return Err(error.into()),
    };

    Ok(HttpResponse::Ok().body(response_body))
}

pub async fn request_ticket_synchronization(
    ticket: Ticket,
    ticket_id: Option<String>,
    destination_id: Option<String>,
    url: String,
) -> Result<Ticket, Box<dyn Error + Send + Sync + 'static>> {
    let client = Client::new();

    let ticket_http_request = TicketHttpRequest {
        ticket,
        ticket_id,
        destination_id,
    };

    let body = match serde_json::to_string_pretty(&ticket_http_request) {
        Ok(json_string) => json_string.as_bytes().to_vec(),
        Err(error) => return Err(error.into()),
    };

    match client.post(url.clone()).body(body).send().await {
        Ok(response) => match response.status() {
            StatusCode::OK => {
                let response_body_bytes = match response.bytes().await {
                    Ok(bytes) => bytes.to_vec(),
                    Err(_) => return Err("failed to receive http response body".into()),
                };

                let response_body_string = match String::from_utf8(response_body_bytes) {
                    Ok(response_body_string) => response_body_string,
                    Err(_) => return Err("failed to read http response body".into()),
                };

                match serde_json::from_str::<TicketHttpResponse>(&response_body_string) {
                    Ok(response) => Ok(response.ticket),
                    Err(_) => Err("failed to parse http response body".into()),
                }
            }
            _ => Err(format!("HTTP Status: {}", response.status()).into()),
        },
        Err(error) => Err(format!("HTTP Failure: {}", error).into()),
    }
}
