use stembot_rust::{adapters::process::Process, init_logger};

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    init_logger("info".to_string());

    let mut p = Process::new("/usr/bin/ls", vec!["-lsah"].as_ref())?;
    let (o, e) = p.read().await?;
    println!("- Output -----\n{o}\n- Error ------\n{e}");

    let mut p = Process::new("/usr/bin/ls", vec!["-lsah", "/asd/"].as_ref())?;
    let (s, o, e) = p.wait_with_output().await?;
    println!("- Status -----\n{s}\n- Output -----\n{o}\n- Error ------\n{e}");

    p.kill().await?;

    Ok(())
}
