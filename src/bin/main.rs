use std::time::Duration;

use clokwerk::{AsyncScheduler, Interval::Seconds};
use tokio::time::sleep;

use stembot_rust::{config::Configuration, init_logger};

async fn schedule_test() {
    log::info!("scheduler test");
}

#[tokio::main]
async fn main() {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    init_logger();

    log::info!("Starting stembot...");

    let configuration = Configuration::new_from_cli();

    log::info!("{}", format!("{:?}", configuration));

    let mut scheduler = AsyncScheduler::new();

    scheduler.every(Seconds(1)).run(schedule_test);

    loop {
        scheduler.run_pending().await;
        sleep(Duration::from_millis(10)).await;
    }
}
