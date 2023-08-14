use actix_web::Result;
use stembot_rust::{
    core::messaging::Ticket, init_logger, private::http::ticketing::request_ticket_synchronization,
};

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    init_logger("info".to_string());

    let ticket = Ticket::Test;
    let ticket_id = None;
    let destination_id = Some(String::from("docker-bot4"));
    let url = String::from("http://127.0.0.1:8090/ticket");

    match request_ticket_synchronization(ticket, ticket_id, destination_id, url).await {
        Ok(ticket) => log::info!("{ticket:?}"),
        Err(error) => log::error!("{error}"),
    };

    Ok(())
}
