use crate::messaging::{TicketRequest, TicketResponse, Ticket};

pub async fn process_ticket_request(ticket_request: TicketRequest) -> TicketResponse {
    match ticket_request.ticket {
        Ticket::Test => TicketResponse { ticket: ticket_request.ticket, ticket_id: ticket_request.ticket_id }
    }
}

pub async fn process_ticket_response(ticket_response: TicketResponse) {
    match ticket_response.ticket {
        Ticket::Test => {}
    }
}
