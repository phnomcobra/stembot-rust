use std::time::Duration;

use stembot_rust::{
    core::messaging::{Ticket, Trace},
    init_logger,
    private::http::client::ticketing::request_ticket_synchronization,
};
use tokio::time::sleep;

async fn test() -> anyhow::Result<()> {
    request_ticket_synchronization(
        Ticket::Test,
        None,
        Some(String::from("docker-bot4")),
        String::from("http://127.0.0.1:8090/ticket/sync"),
    )
    .await?;

    Ok(())
}

async fn trace() -> anyhow::Result<()> {
    let mut ticket = Ticket::BeginTrace(Trace {
        events: vec![],
        period: None,
        destination_id: String::from("docker-bot4"),
        request_id: None,
    });

    let url = String::from("http://127.0.0.1:8090/ticket/sync");

    ticket = request_ticket_synchronization(ticket, None, None, url.clone()).await?;

    sleep(Duration::from_millis(5000)).await;

    if let Ticket::BeginTrace(ticket) = ticket {
        let mut ticket = Ticket::DrainTrace(ticket);

        ticket = request_ticket_synchronization(ticket, None, None, url.clone()).await?;

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

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    init_logger("info".to_string());

    test().await?;
    trace().await?;

    Ok(())
}
