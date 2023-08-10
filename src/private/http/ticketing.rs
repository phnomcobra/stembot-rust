use actix_web::{web, Responder};
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
    ticket_post_request: web::Json<TicketHttpRequest>,
    singleton: web::Data<Singleton>,
) -> actix_web::Result<impl Responder> {
    let singleton = singleton.get_ref();

    let ticket = match synchronize_ticket(
        ticket_post_request.ticket.clone(),
        ticket_post_request.ticket_id.clone(),
        ticket_post_request.destination_id.clone(),
        singleton.clone(),
    )
    .await
    {
        Ok(ticket) => ticket,
        Err(error) => return Err(actix_web::error::ErrorRequestTimeout(error.to_string())),
    };

    Ok(web::Json(TicketHttpResponse { ticket }))
}

pub async fn request_ticket_synchronization(
    ticket: Ticket,
    ticket_id: Option<String>,
    destination_id: Option<String>,
    singleton: Singleton,
) -> Result<Ticket, Box<dyn Error + Send + Sync + 'static>> {
    let singleton = singleton.clone();

    let url = format!(
        "http://{}:{}/{}",
        &singleton.configuration.private_http.host,
        &singleton.configuration.private_http.port,
        &singleton.configuration.private_http.ticket_endpoint
    );

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
                    Err(_) => return Err("failed to parse http response body".into()),
                }
            }
            _ => Err(format!("HTTP Status: {}", response.status()).into()),
        },
        Err(error) => Err(format!("HTTP Failure: {}", error).into()),
    }
}
