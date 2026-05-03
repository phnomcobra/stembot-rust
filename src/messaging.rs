//! Message queue and forwarding system for network message delivery.
//!
//! Mirrors Python's `stembot/messaging.py`.
//!
//! Manages the in-memory message queue for messages destined to this agent and
//! handles routing and forwarding of messages to other agents.  Supports both
//! direct delivery to peers and multi-hop gateway delivery.  Automatically
//! expires old messages.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::collections::{open_messages, open_peers, open_routes};
use crate::executor::agent::AgentClient;
use crate::models::network::{NetworkMessageVariant, NetworkMessagesRequest};
use crate::state::config;

fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

// ── Queue operations ──────────────────────────────────────────────────────────

/// Add a message to the in-memory message queue.
///
/// Mirrors `push_network_message(message)`.
pub fn push_network_message(message: NetworkMessageVariant) -> Result<()> {
    log::debug!("push_network_message");
    open_messages()?.upsert_object(message)?;
    Ok(())
}

/// Retrieve all messages destined for an agent and messages routed through it.
///
/// Returns messages whose `dest` matches `request.isrc` **plus** messages
/// destined for agents that route through `request.isrc` as a gateway.
///
/// Mirrors `pull_network_messages(message)`.
pub fn pull_network_messages(
    request: &NetworkMessagesRequest,
) -> Result<Vec<NetworkMessageVariant>> {
    let isrc = request.isrc.as_deref().unwrap_or("");

    // Build best-gateway map: agtuuid -> (weight, gtwuuid)
    let mut gateway_map: std::collections::HashMap<String, (i64, String)> =
        std::collections::HashMap::new();
    for obj in open_routes()?.find(&[])? {
        let agtuuid = obj.object.agtuuid.clone();
        let weight  = obj.object.weight;
        let gtwuuid = obj.object.gtwuuid.clone();
        match gateway_map.get(&agtuuid) {
            Some((w, _)) if *w <= weight => {}
            _ => { gateway_map.insert(agtuuid, (weight, gtwuuid)); }
        }
    }

    // Collect agent UUIDs: self + agents that route through self as gateway
    let mut agtuuids: Vec<String> = vec![isrc.to_string()];
    for (agtuuid, (_, gtwuuid)) in &gateway_map {
        if gtwuuid == isrc {
            agtuuids.push(agtuuid.clone());
        }
    }

    // Pop all matching messages
    let messages = open_messages()?;
    let mut result = Vec::new();
    for agtuuid in &agtuuids {
        for obj in messages.pop(&[("dest", agtuuid.as_str())])? {
            result.push(obj.object);
        }
    }
    Ok(result)
}

/// Remove and return messages matching the specified criteria.
///
/// Mirrors `pop_network_messages(**kwargs)`.
pub fn pop_network_messages(queries: &[(&str, &str)]) -> Result<Vec<NetworkMessageVariant>> {
    Ok(open_messages()?
        .pop(queries)?
        .into_iter()
        .map(|o| o.object)
        .collect())
}

// ── Forwarding ────────────────────────────────────────────────────────────────

/// Forward a message to its destination via direct delivery or gateway routing.
///
/// Tries direct delivery first; falls back to best-gateway forwarding; re-queues
/// if delivery fails or no route is available.
///
/// Mirrors `forward_network_message(message)`.
pub async fn forward_network_message(message: NetworkMessageVariant) -> Result<()> {
    let dest = dest_of(&message);
    let peers  = open_peers()?;
    let routes = open_routes()?;

    // ── Try direct delivery ───────────────────────────────────────────────────
    let direct = peers.find(&[("agtuuid", dest.as_str()), ("url", "$!eq:null")])?;
    if let Some(peer_obj) = direct.first() {
        if let Some(url) = peer_obj.object.url.clone() {
            let client = AgentClient::with_credentials(
                url.clone(), config().key(), config().agtuuid.clone(),
            );
            match client.send_network_message(message.clone()).await {
                Ok(resp) => log_ack_error(&resp),
                Err(e) => {
                    log::error!("Failed to send to {}: {}", url, e);
                    push_network_message(message)?;
                }
            }
            return Ok(());
        }
    }

    // ── Find best gateway ─────────────────────────────────────────────────────
    let mut best_weight: Option<i64> = None;
    let mut best_gtwuuid: Option<String> = None;
    for obj in routes.find(&[("agtuuid", dest.as_str())])? {
        let w = obj.object.weight;
        if best_weight.map_or(true, |bw| w < bw) {
            best_weight  = Some(w);
            best_gtwuuid = Some(obj.object.gtwuuid.clone());
        }
    }

    if let Some(ref gtwuuid) = best_gtwuuid {
        let gtw_peers = peers.find(&[("agtuuid", gtwuuid.as_str()), ("url", "$!eq:null")])?;
        if let Some(peer_obj) = gtw_peers.first() {
            if let Some(url) = peer_obj.object.url.clone() {
                let client = AgentClient::with_credentials(
                    url.clone(), config().key(), config().agtuuid.clone(),
                );
                match client.send_network_message(message.clone()).await {
                    Ok(resp) => log_ack_error(&resp),
                    Err(e) => {
                        log::error!("Failed to send to gateway {}: {}", url, e);
                        push_network_message(message)?;
                    }
                }
                return Ok(());
            }
        }
    }

    // No route found — re-queue for later delivery
    push_network_message(message)?;
    Ok(())
}

// ── Expiry ────────────────────────────────────────────────────────────────────

/// Remove messages that have exceeded the configured timeout period.
///
/// Mirrors the `@scheduled expire_network_messages()` in Python.
pub fn expire_network_messages() -> Result<()> {
    let cutoff = unix_now() - config().message_timeout_secs as f64;
    for obj in open_messages()?.pop(&[("timestamp", &format!("$lt:{}", cutoff))])? {
        log::warn!("expiring message: {:?}", obj.object);
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract the `dest` field from any `NetworkMessageVariant`.
fn dest_of(msg: &NetworkMessageVariant) -> String {
    match msg {
        NetworkMessageVariant::Ping(m)                => m.dest.clone().unwrap_or_default(),
        NetworkMessageVariant::MessagesRequest(m)     => m.dest.clone().unwrap_or_default(),
        NetworkMessageVariant::MessagesResponse(m)    => m.dest.clone().unwrap_or_default(),
        NetworkMessageVariant::Acknowledgement(m)     => m.dest.clone().unwrap_or_default(),
        NetworkMessageVariant::Advertisement(m)       => m.dest.clone().unwrap_or_default(),
        NetworkMessageVariant::TicketTraceResponse(m) => m.dest.clone().unwrap_or_default(),
        NetworkMessageVariant::TicketRequest(m)       => m.dest.clone().unwrap_or_default(),
        NetworkMessageVariant::TicketResponse(m)      => m.dest.clone().unwrap_or_default(),
    }
}

/// Log the error field if the response is an Acknowledgement with an error.
fn log_ack_error(resp: &NetworkMessageVariant) {
    if let NetworkMessageVariant::Acknowledgement(ack) = resp {
        if let Some(ref err) = ack.error {
            log::error!("acknowledgement error: {}", err);
        }
    }
}
