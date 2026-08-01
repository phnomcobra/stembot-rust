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
use crate::models::network::{NetworkMessage, NetworkMessagesRequest};
use crate::config::config;

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
pub fn push_network_message(message: NetworkMessage) -> Result<()> {
    log::debug!("{}", message.message_type());
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
) -> Result<Vec<NetworkMessage>> {
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

    // Pop all matching messages, respecting optional limit
    let messages = open_messages()?;
    let mut result = Vec::new();
    for agtuuid in &agtuuids {
        let popped = match request.limit {
            Some(lim) => {
                let remaining = lim as usize - result.len();
                if remaining == 0 { break; }
                messages.pop_limited(&[("dest", agtuuid.as_str())], remaining)?
            }
            None => messages.pop(&[("dest", agtuuid.as_str())])?,
        };
        for obj in popped {
            result.push(obj.object);
        }
    }

    Ok(result)
}

/// Filter network messages based on the whitelists in `request`.
///
/// Messages dropped by either whitelist generate a `TicketResponse` error that
/// is re-queued for the requester.
///
/// Mirrors `filter_network_messages(message, network_messages)`.
pub fn filter_network_messages(
    request: &NetworkMessagesRequest,
    mut messages: Vec<NetworkMessage>,
) -> Result<Vec<NetworkMessage>> {
    // Apply network message whitelist if provided in the request.
    if let Some(ref whitelist) = request.network_whitelist {
        log::debug!("Applying network message whitelist: {:?}", whitelist);
        let (allowed, error_tickets) = apply_network_whitelist(messages, whitelist);
        messages = allowed;
        for ticket in error_tickets {
            push_network_message(ticket)?;
        }
    }

    // Apply control form whitelist if provided in the request.
    if let Some(ref whitelist) = request.control_whitelist {
        log::debug!("Applying control form whitelist: {:?}", whitelist);
        let (allowed, error_tickets) = apply_control_whitelist(messages, whitelist);
        messages = allowed;
        for ticket in error_tickets {
            push_network_message(ticket)?;
        }
    }

    Ok(messages)
}

/// Retrieve and filter network messages based on the provided request.
///
/// Repeatedly pulls batches via [`pull_network_messages`] and filters each via
/// [`filter_network_messages`] until the requested `limit` is met or the queue
/// is drained, since whitelist-dropped messages don't count toward the limit.
///
/// Mirrors `pull_filtered_network_messages(message)`.
pub fn pull_filtered_network_messages(
    request: &NetworkMessagesRequest,
) -> Result<Vec<NetworkMessage>> {
    let mut filtered = Vec::new();
    loop {
        let messages = pull_network_messages(request)?;
        if messages.is_empty() {
            break;
        }
        filtered.extend(filter_network_messages(request, messages)?);
        if let Some(limit) = request.limit {
            if filtered.len() as u64 >= limit {
                break;
            }
        }
    }
    Ok(filtered)
}

/// Remove and return messages matching the specified criteria.
///
/// Mirrors `pop_network_messages(**kwargs)`.
pub fn pop_network_messages(queries: &[(&str, &str)]) -> Result<Vec<NetworkMessage>> {
    Ok(open_messages()?
        .pop(queries)?
        .into_iter()
        .map(|o| o.object)
        .collect())
}

// ── Whitelist filtering ───────────────────────────────────────────────────────

/// Filter messages by network message type whitelist.
///
/// Messages whose type is not in `whitelist` are dropped; any dropped
/// `TicketRequest` generates a `TicketResponse` error that must be re-queued.
///
/// Returns `(allowed, error_responses)`.
fn apply_network_whitelist(
    messages: Vec<NetworkMessage>,
    whitelist: &[String],
) -> (Vec<NetworkMessage>, Vec<NetworkMessage>) {
    let mut allowed = Vec::new();
    let mut errors  = Vec::new();
    for msg in messages {
        if whitelist.iter().any(|w| w == msg.message_type()) {
            allowed.push(msg);
        } else {
            if let NetworkMessage::TicketRequest(ref ticket) = msg {
                let mut err  = ticket.clone();
                let old_src  = err.src.clone();
                let old_dest = err.dest.clone().unwrap_or_default();
                err.src  = old_dest;
                err.dest = Some(old_src);
                err.error = Some(format!(
                    "Network message type '{}' is not allowed by whitelist.",
                    msg.message_type()
                ));
                errors.push(NetworkMessage::TicketResponse(err));
            }
        }
    }
    (allowed, errors)
}

/// Filter messages by control form type whitelist.
///
/// For `TicketRequest` messages whose form type is not in `whitelist`, the
/// message is dropped and a `TicketResponse` error is generated for re-queuing.
/// All other message types pass through unchanged.
///
/// Returns `(allowed, error_responses)`.
fn apply_control_whitelist(
    messages: Vec<NetworkMessage>,
    whitelist: &[String],
) -> (Vec<NetworkMessage>, Vec<NetworkMessage>) {
    let mut allowed = Vec::new();
    let mut errors  = Vec::new();
    for msg in messages {
        match &msg {
            NetworkMessage::TicketRequest(ticket) => {
                let form_type = ticket.form.form_type();
                if whitelist.iter().any(|w| w == form_type) {
                    allowed.push(msg);
                } else {
                    let mut err  = ticket.clone();
                    let old_src  = err.src.clone();
                    let old_dest = err.dest.clone().unwrap_or_default();
                    err.src  = old_dest;
                    err.dest = Some(old_src);
                    err.error = Some(format!(
                        "Control form type '{}' is not allowed by whitelist.",
                        form_type
                    ));
                    errors.push(NetworkMessage::TicketResponse(err));
                }
            }
            _ => allowed.push(msg),
        }
    }
    (allowed, errors)
}

// ── Forwarding ────────────────────────────────────────────────────────────────

/// Forward a message to its destination via direct delivery or gateway routing.
///
/// Tries direct delivery first; falls back to best-gateway forwarding; re-queues
/// if delivery fails or no route is available.
///
/// Mirrors `forward_network_message(message)`.
pub async fn forward_network_message(message: NetworkMessage) -> Result<()> {
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
        if best_weight.is_none_or(|bw| w < bw) {
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
        log::warn!("expiring message: {}", obj.object.message_type());
        log::debug!("{:?}", obj.object);
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract the `dest` field from any `NetworkMessageVariant`.
fn dest_of(msg: &NetworkMessage) -> String {
    match msg {
        NetworkMessage::Ping(m)                => m.dest.clone().unwrap_or_default(),
        NetworkMessage::MessagesRequest(m)     => m.dest.clone().unwrap_or_default(),
        NetworkMessage::MessagesResponse(m)    => m.dest.clone().unwrap_or_default(),
        NetworkMessage::Acknowledgement(m)     => m.dest.clone().unwrap_or_default(),
        NetworkMessage::Advertisement(m)       => m.dest.clone().unwrap_or_default(),
        NetworkMessage::TicketTraceResponse(m) => m.dest.clone().unwrap_or_default(),
        NetworkMessage::TicketRequest(m)       => m.dest.clone().unwrap_or_default(),
        NetworkMessage::TicketResponse(m)      => m.dest.clone().unwrap_or_default(),
    }
}

/// Log the error field if the response is an Acknowledgement with an error.
fn log_ack_error(resp: &NetworkMessage) {
    if let NetworkMessage::Acknowledgement(ack) = resp {
        if let Some(ref err) = ack.error {
            log::error!("acknowledgement error: {}", err);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::control::{CommandArg, ControlForm, SyncProcess};
    use crate::models::network::{NetworkTicket, Ping};

    fn make_ping(src: &str, dest: &str) -> NetworkMessage {
        NetworkMessage::Ping(Ping {
            src: src.into(),
            dest: Some(dest.into()),
            isrc: Some(src.into()),
            timestamp: Some(1000.0),
            objuuid: None,
            coluuid: None,
        })
    }

    fn make_sync_process_ticket(src: &str, dest: &str) -> NetworkMessage {
        NetworkMessage::TicketRequest(NetworkTicket {
            tckuuid: "tck-1".into(),
            form: ControlForm::SyncProcess(SyncProcess {
                command: CommandArg::Single("echo hi".into()),
                timeout: 15,
                stdout: None, stderr: None, status: None,
                start_time: None, elapsed_time: None, error: None,
                objuuid: None, coluuid: None,
            }),
            tracing: false,
            src: src.into(),
            dest: Some(dest.into()),
            isrc: Some(src.into()),
            timestamp: Some(1000.0),
            create_time: None, service_time: None, error: None,
            objuuid: None, coluuid: None,
        })
    }

    // ── apply_network_whitelist ───────────────────────────────────────────────

    #[test]
    fn test_network_whitelist_allows_matching_type() {
        let ping   = make_ping("origin", "local");
        let ticket = make_sync_process_ticket("origin", "local");
        let whitelist = vec!["ping".to_string()];

        let (allowed, errors) = apply_network_whitelist(vec![ping, ticket], &whitelist);

        assert_eq!(allowed.len(), 1);
        assert!(matches!(allowed[0], NetworkMessage::Ping(_)));
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], NetworkMessage::TicketResponse(_)));
    }

    #[test]
    fn test_network_whitelist_error_ticket_has_swapped_src_dest() {
        let ticket    = make_sync_process_ticket("origin", "local");
        let whitelist = vec!["ping".to_string()];

        let (allowed, errors) = apply_network_whitelist(vec![ticket], &whitelist);

        assert_eq!(allowed.len(), 0);
        assert_eq!(errors.len(), 1);

        if let NetworkMessage::TicketResponse(err) = &errors[0] {
            assert_eq!(err.src, "local");
            assert_eq!(err.dest.as_deref(), Some("origin"));
            assert!(err.error.as_deref().unwrap().contains("not allowed by whitelist"));
        } else {
            panic!("expected TicketResponse");
        }
    }

    #[test]
    fn test_network_whitelist_non_ticket_dropped_silently() {
        let ping      = make_ping("origin", "local");
        let whitelist = vec!["ticket_request".to_string()];

        let (allowed, errors) = apply_network_whitelist(vec![ping], &whitelist);

        assert_eq!(allowed.len(), 0);
        assert_eq!(errors.len(), 0);
    }

    // ── apply_control_whitelist ───────────────────────────────────────────────

    #[test]
    fn test_control_whitelist_filters_disallowed_form_and_enqueues_error() {
        let ticket    = make_sync_process_ticket("origin", "local");
        let whitelist = vec!["get_peers".to_string()];

        let (allowed, errors) = apply_control_whitelist(vec![ticket], &whitelist);

        assert_eq!(allowed.len(), 0);
        assert_eq!(errors.len(), 1);

        if let NetworkMessage::TicketResponse(err) = &errors[0] {
            assert_eq!(err.src, "local");
            assert_eq!(err.dest.as_deref(), Some("origin"));
            assert!(err.error.as_deref().unwrap().contains("not allowed by whitelist"));
        } else {
            panic!("expected TicketResponse");
        }
    }

    #[test]
    fn test_control_whitelist_allows_matching_form() {
        let ticket    = make_sync_process_ticket("origin", "local");
        let whitelist = vec!["sync_process".to_string()];

        let (allowed, errors) = apply_control_whitelist(vec![ticket], &whitelist);

        assert_eq!(allowed.len(), 1);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_control_whitelist_passes_non_ticket_messages_through() {
        let ping      = make_ping("origin", "local");
        let whitelist = vec!["get_peers".to_string()];

        let (allowed, errors) = apply_control_whitelist(vec![ping], &whitelist);

        assert_eq!(allowed.len(), 1);
        assert_eq!(errors.len(), 0);
    }

    // ── pull_filtered_network_messages ────────────────────────────────────────

    #[test]
    fn test_pull_filtered_network_messages_network_whitelist_enqueues_error() {
        let dest = "pfnm-network-whitelist-dest";
        push_network_message(make_ping("origin", dest)).unwrap();
        push_network_message(make_sync_process_ticket("origin", dest)).unwrap();

        let request = NetworkMessagesRequest {
            src: "origin".into(),
            isrc: Some(dest.into()),
            network_whitelist: Some(vec!["ping".to_string()]),
            ..Default::default()
        };

        let messages = pull_filtered_network_messages(&request).unwrap();
        assert_eq!(messages.len(), 1);
        assert!(matches!(messages[0], NetworkMessage::Ping(_)));

        let pending = open_messages().unwrap().pop(&[("dest", "origin")]).unwrap();
        assert_eq!(pending.len(), 1);
        if let NetworkMessage::TicketResponse(err) = &pending[0].object {
            assert_eq!(err.src, dest);
            assert_eq!(err.dest.as_deref(), Some("origin"));
            assert!(err.error.as_deref().unwrap().contains("not allowed by whitelist"));
        } else {
            panic!("expected TicketResponse");
        }
    }

    #[test]
    fn test_pull_filtered_network_messages_control_whitelist_enqueues_error() {
        let dest = "pfnm-control-whitelist-dest";
        push_network_message(make_sync_process_ticket("origin", dest)).unwrap();

        let request = NetworkMessagesRequest {
            src: "origin".into(),
            isrc: Some(dest.into()),
            control_whitelist: Some(vec!["get_peers".to_string()]),
            ..Default::default()
        };

        let messages = pull_filtered_network_messages(&request).unwrap();
        assert!(messages.is_empty());

        let pending = open_messages().unwrap().pop(&[("dest", "origin")]).unwrap();
        assert_eq!(pending.len(), 1);
        if let NetworkMessage::TicketResponse(err) = &pending[0].object {
            assert_eq!(err.src, dest);
            assert_eq!(err.dest.as_deref(), Some("origin"));
            assert!(err.error.as_deref().unwrap().contains("not allowed by whitelist"));
        } else {
            panic!("expected TicketResponse");
        }
    }
}
