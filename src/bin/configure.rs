use clap::Parser;
use serde_json::json;
use stembot_rust::{
    dao::kvstore::KVStore,
    models::config::LogLevel,
};

#[derive(Parser, Debug)]
#[clap(name = "agt-configure", about = "Configure offline agent settings")]
struct Args {
    #[clap(short = 'a', long, help = "Agent identifier")]
    agtuuid: Option<String>,

    #[clap(short = 'p', long, help = "Server TCP port")]
    port: Option<u16>,

    #[clap(short = 'd', long, help = "Server host address")]
    host: Option<String>,

    #[clap(short = 's', long, help = "Encryption key (will be hashed with SHA-256)")]
    secret: Option<String>,

    #[clap(short = 'l', long, help = "Log output path")]
    log_path: Option<String>,

    #[clap(short = 'c', long = "client-url", help = "Set the agent client control URL")]
    client_url: Option<String>,

    #[clap(short = 'w', long, help = "Number of worker processes")]
    workers: Option<u32>,

    #[clap(long, help = "Application log level (DEBUG/INFO/WARNING/ERROR/CRITICAL)")]
    log_level_app: Option<String>,

    #[clap(long, help = "API/framework log level (DEBUG/INFO/WARNING/ERROR/CRITICAL)")]
    log_level_api: Option<String>,

    #[clap(long, help = "Seconds before an unresponsive peer is considered dead")]
    peer_timeout_secs: Option<u32>,

    #[clap(long, help = "Seconds between peer refresh cycles")]
    peer_refresh_secs: Option<u32>,

    #[clap(long, help = "Maximum route weight for routing decisions")]
    max_weight: Option<u32>,

    #[clap(long, help = "Seconds before a ticket is considered expired")]
    ticket_timeout_secs: Option<u32>,

    #[clap(long, help = "Seconds before a pending message is discarded")]
    message_timeout_secs: Option<u32>,

    #[clap(long, help = "Set client control URL to local host (http://127.0.0.1:<port>/control)")]
    client_local: bool,

    #[clap(short = 'v', long, help = "View current configuration settings")]
    view: bool,

    #[clap(short = 'e', long = "load-env", help = "Load configuration from environment variables")]
    load_env: bool,
}

fn load_from_environment(store: &KVStore) {
    if let Ok(v) = std::env::var("AGT_UUID") {
        store.commit("agtuuid", json!(v.clone())).unwrap();
        println!("✓ Loaded AGT_UUID: {v}");
    }
    if let Ok(v) = std::env::var("AGT_HOST") {
        store.commit("socket_host", json!(v.clone())).unwrap();
        println!("✓ Loaded AGT_HOST: {v}");
    }
    if let Ok(v) = std::env::var("AGT_PORT") {
        if let Ok(port) = v.parse::<u16>() {
            store.commit("socket_port", json!(port)).unwrap();
            println!("✓ Loaded AGT_PORT: {v}");
        }
    }
    if let Ok(v) = std::env::var("AGT_LOG_PATH") {
        store.commit("log_path", json!(v.clone())).unwrap();
        println!("✓ Loaded AGT_LOG_PATH: {v}");
    }
    if let Ok(v) = std::env::var("AGT_SECRET") {
        let digest = sha256::digest(v.as_str());
        store.commit("secret_digest", json!(digest)).unwrap();
        println!("✓ Loaded AGT_SECRET (hashed to 32 bytes)");
    }
    if let Ok(v) = std::env::var("AGT_CLIENT_CONTROL_URL") {
        store.commit("client_control_url", json!(v.clone())).unwrap();
        println!("✓ Loaded AGT_CLIENT_CONTROL_URL: {v}");
    }
    if let Ok(v) = std::env::var("AGT_WORKERS") {
        if let Ok(w) = v.parse::<u32>() {
            store.commit("workers", json!(w)).unwrap();
            println!("✓ Loaded AGT_WORKERS: {v}");
        }
    }
    if let Ok(v) = std::env::var("AGT_LOG_LEVEL_APP") {
        if v.to_uppercase().parse::<LogLevel>().is_ok() {
            store.commit("log_level_app", json!(v.to_uppercase())).unwrap();
            println!("✓ Loaded AGT_LOG_LEVEL_APP: {v}");
        }
    }
    if let Ok(v) = std::env::var("AGT_LOG_LEVEL_API") {
        if v.to_uppercase().parse::<LogLevel>().is_ok() {
            store.commit("log_level_api", json!(v.to_uppercase())).unwrap();
            println!("✓ Loaded AGT_LOG_LEVEL_API: {v}");
        }
    }
    if let Ok(v) = std::env::var("AGT_PEER_TIMEOUT_SECS") {
        if let Ok(n) = v.parse::<u32>() {
            store.commit("peer_timeout_secs", json!(n)).unwrap();
            println!("✓ Loaded AGT_PEER_TIMEOUT_SECS: {v}");
        }
    }
    if let Ok(v) = std::env::var("AGT_PEER_REFRESH_SECS") {
        if let Ok(n) = v.parse::<u32>() {
            store.commit("peer_refresh_secs", json!(n)).unwrap();
            println!("✓ Loaded AGT_PEER_REFRESH_SECS: {v}");
        }
    }
    if let Ok(v) = std::env::var("AGT_MAX_WEIGHT") {
        if let Ok(n) = v.parse::<u32>() {
            store.commit("max_weight", json!(n)).unwrap();
            println!("✓ Loaded AGT_MAX_WEIGHT: {v}");
        }
    }
    if let Ok(v) = std::env::var("AGT_TICKET_TIMEOUT_SECS") {
        if let Ok(n) = v.parse::<u32>() {
            store.commit("ticket_timeout_secs", json!(n)).unwrap();
            println!("✓ Loaded AGT_TICKET_TIMEOUT_SECS: {v}");
        }
    }
    if let Ok(v) = std::env::var("AGT_MESSAGE_TIMEOUT_SECS") {
        if let Ok(n) = v.parse::<u32>() {
            store.commit("message_timeout_secs", json!(n)).unwrap();
            println!("✓ Loaded AGT_MESSAGE_TIMEOUT_SECS: {v}");
        }
    }
}

fn display_config(store: &KVStore) {
    let v = |key: &str| -> String {
        store.get(key, None)
            .ok()
            .and_then(|j| if j.is_null() { None } else { Some(j.to_string().trim_matches('"').to_string()) })
            .unwrap_or_else(|| String::from("(not set)"))
    };
    println!("\n{}", "=".repeat(50));
    println!("Current Configuration");
    println!("{}", "=".repeat(50));
    let items = [
        ("Client Control URL",   v("client_control_url")),
        ("Agent ID",             v("agtuuid")),
        ("Socket Host",          v("socket_host")),
        ("Socket Port",          v("socket_port")),
        ("Workers",              v("workers")),
        ("Log Path",             v("log_path")),
        ("Log Level App",        v("log_level_app")),
        ("Log Level API",        v("log_level_api")),
        ("Peer Timeout Secs",    v("peer_timeout_secs")),
        ("Peer Refresh Secs",    v("peer_refresh_secs")),
        ("Max Weight",           v("max_weight")),
        ("Ticket Timeout Secs",  v("ticket_timeout_secs")),
        ("Message Timeout Secs", v("message_timeout_secs")),
        ("Secret Digest",        v("secret_digest")),
    ];
    for (label, value) in &items {
        let display = if value.len() > 60 { format!("{}...", &value[..60]) } else { value.clone() };
        println!("{label:.<25} {display}");
    }
    println!("{}", "=".repeat(50));
    println!();
}

fn main() {
    let args = Args::parse();
    let store = KVStore::new(None).expect("failed to open kvstore");
    let mut modified = false;

    if args.load_env {
        println!("Loading configuration from environment variables...");
        load_from_environment(&store);
        modified = true;
    }

    if let Some(v) = args.agtuuid {
        store.commit("agtuuid", json!(v.clone())).unwrap();
        println!("✓ Set Agent UUID: {v}");
        modified = true;
    }
    if let Some(v) = args.host {
        store.commit("socket_host", json!(v.clone())).unwrap();
        println!("✓ Set Host: {v}");
        modified = true;
    }
    if let Some(v) = args.port {
        store.commit("socket_port", json!(v)).unwrap();
        println!("✓ Set Port: {v}");
        modified = true;
    }
    if let Some(v) = args.log_path {
        store.commit("log_path", json!(v.clone())).unwrap();
        println!("✓ Set Log Path: {v}");
        modified = true;
    }
    if let Some(v) = args.secret {
        let digest = sha256::digest(v.as_str());
        store.commit("secret_digest", json!(digest)).unwrap();
        println!("✓ Set Secret (hashed to 32 bytes)");
        modified = true;
    }
    if let Some(v) = args.client_url {
        store.commit("client_control_url", json!(v.clone())).unwrap();
        println!("✓ Set Client Control URL: {v}");
        modified = true;
    }
    if let Some(v) = args.workers {
        store.commit("workers", json!(v)).unwrap();
        println!("✓ Set Workers: {v}");
        modified = true;
    }
    if let Some(v) = args.log_level_app {
        let upper = v.to_uppercase();
        if upper.parse::<LogLevel>().is_ok() {
            store.commit("log_level_app", json!(upper.clone())).unwrap();
            println!("✓ Set Log Level App: {upper}");
            modified = true;
        } else {
            eprintln!("Error: invalid log level '{v}'");
        }
    }
    if let Some(v) = args.log_level_api {
        let upper = v.to_uppercase();
        if upper.parse::<LogLevel>().is_ok() {
            store.commit("log_level_api", json!(upper.clone())).unwrap();
            println!("✓ Set Log Level API: {upper}");
            modified = true;
        } else {
            eprintln!("Error: invalid log level '{v}'");
        }
    }
    if let Some(v) = args.peer_timeout_secs {
        store.commit("peer_timeout_secs", json!(v)).unwrap();
        println!("✓ Set Peer Timeout Secs: {v}");
        modified = true;
    }
    if let Some(v) = args.peer_refresh_secs {
        store.commit("peer_refresh_secs", json!(v)).unwrap();
        println!("✓ Set Peer Refresh Secs: {v}");
        modified = true;
    }
    if let Some(v) = args.max_weight {
        store.commit("max_weight", json!(v)).unwrap();
        println!("✓ Set Max Weight: {v}");
        modified = true;
    }
    if let Some(v) = args.ticket_timeout_secs {
        store.commit("ticket_timeout_secs", json!(v)).unwrap();
        println!("✓ Set Ticket Timeout Secs: {v}");
        modified = true;
    }
    if let Some(v) = args.message_timeout_secs {
        store.commit("message_timeout_secs", json!(v)).unwrap();
        println!("✓ Set Message Timeout Secs: {v}");
        modified = true;
    }
    if args.client_local {
        let port = store.get("socket_port", None)
            .ok()
            .and_then(|j| j.as_u64())
            .unwrap_or(8080) as u16;
        let local_url = format!("http://127.0.0.1:{port}/control");
        store.commit("client_control_url", json!(local_url.clone())).unwrap();
        println!("✓ Set Client Control URL to local: {local_url}");
        modified = true;
    }

    if args.view {
        display_config(&store);
    } else if !modified {
        println!("No options provided. Use --help for usage information.");
    }
}

