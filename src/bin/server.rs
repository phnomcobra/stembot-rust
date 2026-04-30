use actix_web::{rt::spawn, web, App, HttpServer, Result};
use clokwerk::{AsyncScheduler, Interval::Seconds};
use tracing_actix_web::TracingLogger;

use std::time::Duration;
use tokio::time::sleep;

use stembot_rust::{
    state::Singleton,
    processor::test_handler,
    logger::init_logger,
};

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    let singleton = Singleton::default();
    log::info!("{:?}", singleton.configuration);
    

    init_logger("debug".to_string());

    let mut scheduler = AsyncScheduler::new();

    scheduler.every(Seconds(5)).run({
        move || async move {
            log::info!("Scheduler demo");
        }
    });

    
    log::info!("Starting scheduler");
    
    spawn({
        async move {
            loop {
                scheduler.run_pending().await;
                sleep(Duration::from_secs(1)).await;
            }
        }
    });

    let donor = singleton.clone();
    let server = HttpServer::new(
        move || {
            App::new()
                .wrap(TracingLogger::default())
                .app_data(web::Data::new(donor.clone()))
                .route("/test", web::post().to(test_handler),)
        }
    );

    log::info!("Starting server");

    server.bind(
        (
            singleton.configuration.host.clone(),
            singleton.configuration.port
        )
    )?.run().await
}
