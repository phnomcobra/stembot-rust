use stembot_rust::{
    core::{state::Singleton, ticket::TicketMessage},
    init_logger,
    interface::debug::{peer_query, route_query, ticket_query, trace},
    private::http::client::ticketing::{request_ticket_initialization, request_ticket_retrieval},
};

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    init_logger("info".to_string());

    let singleton = Singleton::new_from_cli();

    let url = format!(
        "http://{}:{}{}",
        singleton.configuration.private_http.host,
        singleton.configuration.private_http.port,
        singleton.configuration.private_http.ticket_async_endpoint
    );

    // trace(String::from("docker-bot4"), singleton.clone()).await?;

    let mut ticket_ids_to_receive = vec![];
    for id in [
        "docker-bot0",
        "docker-bot1",
        "docker-bot2",
        "docker-bot3",
        "docker-bot4",
    ] {
        ticket_ids_to_receive.push(
            request_ticket_initialization(
                TicketMessage::Test,
                None,
                Some(String::from(id)),
                url.clone(),
            )
            .await?,
        );
    }

    for id in [
        "docker-bot0",
        "docker-bot1",
        "docker-bot2",
        "docker-bot3",
        "docker-bot4",
    ] {
        log::info!("- Peers --- {id}");
        peer_query(Some(String::from(id)), singleton.clone()).await?;
        log::info!("- Routes -- {id}");
        route_query(Some(String::from(id)), singleton.clone()).await?;
        log::info!("- Tickets - {id}");
        ticket_query(Some(String::from(id)), singleton.clone()).await?;
    }

    for id in ticket_ids_to_receive {
        request_ticket_retrieval(id, url.clone()).await?;
    }

    log::info!("{}", trace(String::from("docker-bot2"), singleton).await?);

    Ok(())
}
