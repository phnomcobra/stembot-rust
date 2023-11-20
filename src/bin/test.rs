use stembot_rust::{
    core::state::Singleton,
    init_logger,
    interface::debug::{peer_query, route_query, trace},
};

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    init_logger("info".to_string());

    let singleton = Singleton::new_from_cli();

    // trace(String::from("docker-bot4"), singleton.clone()).await?;

    for id in [
        "docker-bot0",
        "docker-bot1",
        "docker-bot2",
        "docker-bot3",
        "docker-bot4",
    ] {
        log::info!("- Peers -- {id} ----------------------");
        peer_query(Some(String::from(id)), singleton.clone()).await?;
        log::info!("- Routes - {id} ----------------------");
        route_query(Some(String::from(id)), singleton.clone()).await?;
    }

    Ok(())
}
