use std::time::Duration;

use actix_web::Result;
use stembot_rust::{
    core::messaging::{Ticket, TraceTicket},
    init_logger,
    private::http::ticketing::request_ticket_synchronization,
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

    let mut ticket = Ticket::BeginTrace(TraceTicket {
        events: vec![],
        period: None,
        destination_id: String::from("docker-bot4"),
        request_id: None,
    });

    let destination_id = None;
    let url = String::from("http://127.0.0.1:8090/ticket/sync");

    ticket = request_ticket_synchronization(ticket, None, destination_id.clone(), url.clone())
        .await
        .unwrap();

    sleep(Duration::from_millis(5000)).await;

    if let Ticket::BeginTrace(ticket) = ticket {
        let mut ticket = Ticket::DrainTrace(ticket);

        ticket = request_ticket_synchronization(ticket, None, destination_id, url.clone())
            .await
            .unwrap();

        if let Ticket::DrainTrace(ticket) = ticket {
            let mut events = ticket.events;
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
        };
    };

    Ok(())
}
