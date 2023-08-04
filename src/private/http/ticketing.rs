use actix_web::{web, Responder};
use serde::{Deserialize, Serialize};

use crate::{messaging::Ticket, state::Singleton, ticketing::send_ticket};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketPostRequest {
    pub ticket: Ticket,
    pub ticket_id: Option<String>,
    pub destination_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TicketPostResponse {
    pub ticket_id: String,
}

pub async fn post_ticket(
    ticket_post_request: web::Json<TicketPostRequest>,
    singleton: web::Data<Singleton>,
) -> actix_web::Result<impl Responder> {
    let singleton = singleton.get_ref();

    let ticket_id = send_ticket(
        ticket_post_request.ticket.clone(),
        ticket_post_request.ticket_id.clone(),
        ticket_post_request.destination_id.clone(),
        singleton.clone(),
    )
    .await;

    Ok(web::Json(TicketPostResponse { ticket_id }))
}
