use stembot_rust::{core::state::Singleton, init_logger, interface::debug::trace};

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    init_logger("info".to_string());

    let singleton = Singleton::new_from_cli();

    trace(String::from("docker-bot4"), singleton).await?;

    Ok(())
}
