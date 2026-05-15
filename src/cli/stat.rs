use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use chrono::{SecondsFormat, TimeZone, Utc};

use crate::{
    executor::agent::AgentClient,
    models::control::{ControlForm, ControlFormTicket, GetConfig, GetPeers, GetRoutes},
};

use super::poll_ticket;

pub async fn cmd_stat(
    client: Arc<AgentClient>,
    agtuuid: String,
    timeout: u64,
) -> Result<()> {
    let config_ticket = ControlFormTicket {
        dst: agtuuid.clone(),
        tracing: true,
        form: ControlForm::GetConfig(GetConfig::default()),
        ..ControlFormTicket::default()
    };
    let peers_ticket = ControlFormTicket {
        dst: agtuuid.clone(),
        form: ControlForm::GetPeers(GetPeers::default()),
        ..ControlFormTicket::default()
    };
    let routes_ticket = ControlFormTicket {
        dst: agtuuid.clone(),
        form: ControlForm::GetRoutes(GetRoutes::default()),
        ..ControlFormTicket::default()
    };

    // Send all three initial tickets concurrently
    let (config_res, peers_res, routes_res) = tokio::join!(
        client.send_ticket(config_ticket),
        client.send_ticket(peers_ticket),
        client.send_ticket(routes_ticket),
    );
    let it = Instant::now();

    let config_ticket  = config_res?;
    let peers_ticket   = peers_res?;
    let routes_ticket  = routes_res?;

    let config_ticket  = poll_ticket(Arc::clone(&client), config_ticket, timeout).await;
    let et = config_ticket
        .service_time
        .map(|s| s - config_ticket.create_time)
        .unwrap_or_else(|| it.elapsed().as_secs_f64());

    let peers_ticket   = poll_ticket(Arc::clone(&client), peers_ticket, timeout).await;
    let routes_ticket  = poll_ticket(Arc::clone(&client), routes_ticket, timeout).await;

    for ticket in [&config_ticket, &peers_ticket, &routes_ticket] {
        if let Some(ref e) = ticket.error { eprintln!("{e}"); }
    }

    let config = match &config_ticket.form {
        ControlForm::GetConfig(f) => f.config.clone(),
        _ => None,
    };
    let peers = match &peers_ticket.form {
        ControlForm::GetPeers(f) => f.peers.clone(),
        _ => vec![],
    };
    let mut routes = match &routes_ticket.form {
        ControlForm::GetRoutes(f) => f.routes.clone(),
        _ => vec![],
    };
    routes.sort_by(|a, b| a.agtuuid.cmp(&b.agtuuid));

    let mut hops = config_ticket.hops.clone();
    hops.sort_by(|a, b| a.hop_time.partial_cmp(&b.hop_time).unwrap_or(std::cmp::Ordering::Equal));

    println!();
    println!("{}", "=".repeat(70));
    println!("Agent Statistics: {agtuuid}");
    println!("{}", "=".repeat(70));

    println!();
    println!("Elapsed Time");
    println!("   {et:.3} seconds");

    println!();
    println!("Configuration");
    if let Some(cfg) = config {
        if let Some(obj) = cfg.as_object() {
            for (k, v) in obj {
                let s = v.to_string();
                let display = if s.len() > 60 { format!("{}...", &s[..60]) } else { s };
                println!("   {k:.<36} {display}");
            }
        }
    } else {
        println!("   (No configuration data received)");
    }

    println!();
    println!("Network Peers");
    for peer in &peers {
        println!(
            "   {:.<36} Polling: {:<5} URL: {}",
            peer.agtuuid.as_deref().unwrap_or("(none)"),
            peer.polling,
            peer.url.as_deref().unwrap_or("(none)"),
        );
    }

    println!();
    println!("Network Routes");
    for route in &routes {
        println!(
            "   {:.<36} -> {:.<36} (weight: {})",
            route.agtuuid, route.gtwuuid, route.weight,
        );
    }

    println!();
    println!("Network Hops");
    for (idx, hop) in hops.iter().enumerate() {
        let secs  = hop.hop_time as i64;
        let nanos = ((hop.hop_time - secs as f64) * 1_000_000_000.0) as u32;
        let time_str = Utc
            .timestamp_opt(secs, nanos)
            .single()
            .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Millis, true))
            .unwrap_or_else(|| hop.hop_time.to_string());
        println!(
            "   [{: <2}] {:.<36} {:.<20} @ {}",
            idx + 1,
            hop.agtuuid,
            hop.type_str,
            time_str,
        );
    }

    println!();
    println!("{}", "=".repeat(70));
    println!();
    Ok(())
}
