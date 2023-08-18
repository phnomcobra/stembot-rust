use std::time::Duration;

use actix_web::Result;
use stembot_rust::{
    core::messaging::{Ticket, TraceTicket},
    init_logger,
    private::http::ticketing::{
        request_ticket_initialization, request_ticket_retrieval, request_ticket_synchronization,
    },
};
use tokio::time::sleep;

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    init_logger("info".to_string());

    let ticket = Ticket::Test;
    let ticket_id = None;
    let destination_id = Some(String::from("docker-bot4"));
    let url = String::from("http://127.0.0.1:8090/ticket/sync");

    match request_ticket_synchronization(ticket, ticket_id, destination_id, url).await {
        Ok(ticket) => log::info!("{ticket:?}"),
        Err(error) => log::error!("{error}"),
    };

    let ticket = Ticket::Trace(TraceTicket {
        events: vec![],
        period: 5000,
        destination_id: String::from("docker-bot4"),
    });
    let mut ticket_id = None;
    let destination_id = None;
    let url = String::from("http://127.0.0.1:8090/ticket/async");

    ticket_id = request_ticket_initialization(ticket, ticket_id, destination_id, url.clone())
        .await
        .unwrap();

    sleep(Duration::from_millis(5000)).await;

    match request_ticket_retrieval(ticket_id, url).await {
        Ok(ticket) => match ticket {
            Ticket::Trace(ticket) => {
                let mut events = ticket.events.clone();
                events.sort_by(|a, b| a.hop_count.cmp(&b.hop_count));
                for event in events {
                    log::info!(
                        "{} {}: {}, {}",
                        event.hop_count,
                        event.direction,
                        event.id,
                        event.local_time
                    );
                }
            }
            _ => {}
        },
        Err(error) => log::error!("{error}"),
    };

    Ok(())
}
