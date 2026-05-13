//! Core processing logic for the Stembot agent.
//!
//! Mirrors Python's `stembot/processor.py`.
//!
//! Implements actix-web handlers for the `/control` and `/mpi` endpoints,
//! processes control forms and network messages, and provides scheduled
//! background functions for message replay, peer polling, and route advertisement.
//!
//! Encryption protocol:
//! - Request and response bodies are raw binary AES-256 EAX ciphertext
//!   (Content-Type: application/binary).
//! - The AES nonce and MAC tag are transmitted as hex strings in the
//!   Nonce and Tag HTTP headers respectively.

use actix_web::{web, HttpRequest, HttpResponse, Result as ActixResult};
use serde_json::{json, Value};

use crate::collections::{open_peers, open_tickets};
use crate::config::config;
use crate::executor::agent::{decrypt, encrypt, AgentClient};
use crate::executor::file::{load_file_to_form, write_file_from_form};
use crate::executor::process::sync_process;
use crate::messaging::{forward_network_message, pop_network_messages, pull_network_messages};
use crate::models::config::Config;
use crate::models::control::{
    CheckTicket, CommandArg, ControlFormTicket, ControlForm, SyncProcess as SyncProcessForm,
};
use crate::models::network::{
    Acknowledgement, NetworkMessage, NetworkMessagesRequest, NetworkMessagesResponse,
    NetworkTicket,
};
use crate::peering::{
    age_routes, create_peer, create_route_advertisement, delete_peer, delete_peers, get_peers,
    get_routes, process_route_advertisement, touch_peer,
};
use crate::ticketing::{check_ticket, close_ticket, dedup_trace, read_ticket, service_ticket, service_trace};

// ── HTTP Handlers ─────────────────────────────────────────────────────────────

/// Handler for the `/control` endpoint.
///
/// Receives raw binary AES-256 EAX ciphertext bodies, decrypts using the
/// hex-encoded Nonce and Tag headers, processes, and returns an encrypted
/// response in the same format.
///
/// Mirrors Python's `/control` endpoint.
pub async fn control_handler(
    body: web::Bytes,
    config_data: web::Data<Config>,
    request: HttpRequest,
) -> ActixResult<HttpResponse> {
    let key = config_data.key();

    let nonce = extract_header_hex(&request, "Nonce")?;
    let tag   = extract_header_hex(&request, "Tag")?;
    let ct    = body.to_vec();

    let plaintext = decrypt(&key, &nonce, &tag, &ct)
        .map_err(actix_web::error::ErrorBadRequest)?;

    let raw: Value = serde_json::from_slice(&plaintext)
        .map_err(actix_web::error::ErrorBadRequest)?;

    let raw_response = match raw.get("type").and_then(Value::as_str) {
        Some("create_ticket") | Some("read_ticket") => {
            let ticket: ControlFormTicket = serde_json::from_value(raw)
                .map_err(actix_web::error::ErrorBadRequest)?;
            log::debug!("{}", ticket.form_type);
            let result = process_ticket_form(ticket).await;
            serde_json::to_vec(&result)
                .map_err(actix_web::error::ErrorInternalServerError)?
        }
        _ => {
            let form: ControlForm = serde_json::from_value(raw)
                .map_err(actix_web::error::ErrorBadRequest)?;
            log::debug!("{}", form.form_type());
            let result = process_control_form(form).await;
            serde_json::to_vec(&result)
                .map_err(actix_web::error::ErrorInternalServerError)?
        }
    };

    let (nonce_out, tag_out, ct_out) = encrypt(&key, &raw_response)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok()
        .append_header(("Nonce", hex::encode(nonce_out)))
        .append_header(("Tag", hex::encode(tag_out)))
        .body(ct_out))
}

/// Handler for the `/mpi` endpoint.
///
/// Receives raw binary AES-256 EAX ciphertext bodies, decrypts using the
/// hex-encoded Nonce and Tag headers, routes, and returns an encrypted
/// response in the same format.
///
/// Mirrors Python's `/mpi` endpoint.
pub async fn mpi_handler(
    body: web::Bytes,
    config_data: web::Data<Config>,
    request: HttpRequest,
) -> ActixResult<HttpResponse> {
    let key = config_data.key();

    let nonce = extract_header_hex(&request, "Nonce")?;
    let tag   = extract_header_hex(&request, "Tag")?;
    let ct    = body.to_vec();

    let plaintext = decrypt(&key, &nonce, &tag, &ct)
        .map_err(actix_web::error::ErrorBadRequest)?;

    let mut message: NetworkMessage = serde_json::from_slice(&plaintext)
        .map_err(actix_web::error::ErrorBadRequest)?;

    if let Some(isrc) = isrc_of(&message) {
        touch_peer(&isrc).unwrap_or_else(|e| log::error!("touch_peer error: {e}"));
    }

    if dest_of(&message).is_empty() {
        set_dest(&mut message, config().agtuuid.clone());
    }

    let response = route_network_message(message).await;

    let raw_response = serde_json::to_vec(&response)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let (nonce_out, tag_out, ct_out) = encrypt(&key, &raw_response)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok()
        .append_header(("Nonce", hex::encode(nonce_out)))
        .append_header(("Tag", hex::encode(tag_out)))
        .body(ct_out))
}

// ── Control form processing ───────────────────────────────────────────────────

/// Process a control form by dispatching to the appropriate handler.
///
/// Mirrors Python's `process_control_form(form)`.
pub async fn process_control_form(form: ControlForm) -> ControlForm {
    log::debug!("{}", form.form_type());
    match form {
        ControlForm::DiscoverPeer(mut f) => {
            let client = AgentClient::with_credentials(
                f.url.clone(),
                config().key(),
                config().agtuuid.clone(),
            );
            match client
                .send_network_message(NetworkMessage::Ping(
                    crate::models::network::Ping::default(),
                ))
                .await
            {
                Ok(NetworkMessage::Acknowledgement(ack)) => {
                    if let Some(ref dest) = ack.dest {
                        f.agtuuid = Some(dest.clone());
                        if let Err(e) = create_peer(
                            dest,
                            Some(f.url.clone()),
                            f.ttl.map(|t| t as u32),
                            f.polling,
                        ) {
                            f.error = Some(e.to_string());
                        }
                    }
                }
                Ok(other) => {
                    log::warn!(
                        "unexpected response to ping during peer discovery: {:?}",
                        other
                    );
                }
                Err(e) => {
                    f.error = Some(e.to_string());
                }
            }
            ControlForm::DiscoverPeer(f)
        }

        ControlForm::CreatePeer(mut f) => {
            if let Err(e) =
                create_peer(&f.agtuuid, f.url.clone(), f.ttl.map(|t| t as u32), f.polling)
            {
                f.error = Some(e.to_string());
            }
            ControlForm::CreatePeer(f)
        }

        ControlForm::DeletePeers(mut f) => {
            let result = match f.agtuuids {
                Some(ref ids) => ids.iter().try_for_each(|id| delete_peer(id)),
                None => delete_peers(),
            };
            if let Err(e) = result {
                f.error = Some(e.to_string());
            }
            ControlForm::DeletePeers(f)
        }

        ControlForm::GetPeers(mut f) => {
            match get_peers() {
                Ok(peers) => f.peers = peers,
                Err(e) => f.error = Some(e.to_string()),
            }
            ControlForm::GetPeers(f)
        }

        ControlForm::GetRoutes(mut f) => {
            match get_routes() {
                Ok(routes) => f.routes = routes,
                Err(e) => f.error = Some(e.to_string()),
            }
            ControlForm::GetRoutes(f)
        }

        ControlForm::SyncProcess(f) => {
            match tokio::task::spawn_blocking(move || sync_process(f)).await {
                Ok(result) => ControlForm::SyncProcess(result),
                Err(e) => {
                    log::error!("sync_process task error: {e}");
                    ControlForm::SyncProcess(SyncProcessForm {
                        command:      CommandArg::default(),
                        timeout:      15,
                        stdout:       None,
                        stderr:       None,
                        status:       None,
                        start_time:   None,
                        elapsed_time: None,
                        error:        Some(e.to_string()),
                        objuuid:      None,
                        coluuid:      None,
                    })
                }
            }
        }

        ControlForm::LoadFile(f) => ControlForm::LoadFile(load_file_to_form(f)),

        ControlForm::WriteFile(f) => ControlForm::WriteFile(write_file_from_form(f)),

        ControlForm::GetConfig(mut f) => {
            f.config = Some(config_to_json());
            ControlForm::GetConfig(f)
        }

        ControlForm::CheckTicket(f) => {
            match check_ticket(f) {
                Ok(updated) => ControlForm::CheckTicket(updated),
                Err(e) => ControlForm::CheckTicket(CheckTicket { error: Some(e.to_string()), ..Default::default() }),
            }
        }

        ControlForm::CloseTicket(f) => {
            close_ticket(&f).unwrap_or_else(|e| log::error!("close_ticket error: {e}"));
            ControlForm::CloseTicket(f)
        }
    }
}

/// Handle ticket-type control forms received at the `/control` endpoint.
///
/// Dispatches `create_ticket` and `read_ticket` operations.
async fn process_ticket_form(mut ticket: ControlFormTicket) -> ControlFormTicket {
    log::debug!("{}", ticket.form_type);
    match ticket.form_type.as_str() {
        "create_ticket" => create_form_ticket(ticket).await,
        "read_ticket" => match read_ticket(&ticket) {
            Ok(Some(t)) => t,
            Ok(None)    => ticket,
            Err(e) => {
                ticket.error = Some(e.to_string());
                ticket
            }
        },
        _ => ticket,
    }
}

/// Create a network ticket from a control form ticket and route it to the destination.
///
/// Mirrors Python's `create_form_ticket(control_form_ticket)`.
pub async fn create_form_ticket(control_form_ticket: ControlFormTicket) -> ControlFormTicket {
    let network_ticket = NetworkTicket {
        tckuuid:      control_form_ticket.tckuuid.clone(),
        form:         control_form_ticket.form.clone(),
        tracing:      control_form_ticket.tracing,
        src:          config().agtuuid.clone(),
        dest:         Some(control_form_ticket.dst.clone()),
        isrc:         None,
        timestamp:    None,
        create_time:  Some(control_form_ticket.create_time),
        service_time: None,
        error:        None,
        objuuid:      None,
        coluuid:      None,
    };

    let stored = open_tickets()
        .and_then(|tickets| tickets.upsert_object(control_form_ticket.clone()))
        .map(|obj| obj.object);

    route_network_message(NetworkMessage::TicketRequest(network_ticket)).await;

    match stored {
        Ok(mut ticket) => {
            // Clear file content from the ticket before returning to avoid unnecessary data transfer
            if let ControlForm::WriteFile(ref mut f) = ticket.form {
                f.b64zlib = "".to_string();
            }
            ticket
        },
        Err(e) => {
            log::error!("create_form_ticket: failed to store ticket: {e}");
            control_form_ticket
        }
    }
}

// ── Network message routing ───────────────────────────────────────────────────

/// Route a network message to its destination or forward it to an intermediate peer.
///
/// Mirrors Python's `route_network_message(message)`.
pub fn route_network_message(
    message_in: NetworkMessage,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = NetworkMessage> + Send>> {
    Box::pin(async move {
    // Handle dedup tracing for ticket messages
    let ticket_type = match &message_in {
        NetworkMessage::TicketRequest(_)  => Some("ticket_request"),
        NetworkMessage::TicketResponse(_) => Some("ticket_response"),
        _ => None,
    };

    if let Some(ttype) = ticket_type {
        let ticket = match &message_in {
            NetworkMessage::TicketRequest(t)  => t.clone(),
            NetworkMessage::TicketResponse(t) => t.clone(),
            _ => unreachable!(),
        };
        match dedup_trace(&ticket, ttype) {
            Ok(Some(trace)) => {
                let dest = trace.dest.clone().unwrap_or_default();
                let trace_msg = NetworkMessage::TicketTraceResponse(trace);
                if dest == config().agtuuid {
                    process_network_message(trace_msg).await;
                } else {
                    tokio::spawn(async move {
                        if let Err(e) = forward_network_message(trace_msg).await {
                            log::error!("forward trace error: {e}");
                        }
                    });
                }
            }
            Ok(None) => {}
            Err(e) => log::error!("dedup_trace error: {e}"),
        }
    }

    let dest = dest_of(&message_in);

    if dest == config().agtuuid {
        return match process_network_message(message_in.clone()).await {
            Some(response) => response,
            None => NetworkMessage::Acknowledgement(Acknowledgement {
                ack_type: msg_type_of(&message_in).to_string(),
                src:      src_of(&message_in),
                dest:     Some(dest),
                ..Default::default()
            }),
        };
    }

    let msg = message_in.clone();
    tokio::spawn(async move {
        if let Err(e) = forward_network_message(msg).await {
            log::error!("forward_network_message error: {e}");
        }
    });

    NetworkMessage::Acknowledgement(Acknowledgement {
        ack_type: msg_type_of(&message_in).to_string(),
        src:      src_of(&message_in),
        dest:     Some(dest),
        ..Default::default()
    })
    }) // end Box::pin
}

/// Process a network message based on its type and generate an appropriate response.
///
/// Mirrors Python's `process_network_message(message)`.
fn process_network_message(
    message: NetworkMessage,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<NetworkMessage>> + Send>> {
    log::debug!("{}", message.message_type());
    Box::pin(async move {
    match message {
        NetworkMessage::Ping(_) => None,

        NetworkMessage::Advertisement(adv) => {
            if let Err(e) = process_route_advertisement(&adv) {
                log::error!("process_route_advertisement error: {e}");
            }
            None
        }

        NetworkMessage::TicketRequest(mut ticket) => {
            ticket.form = process_control_form(ticket.form).await;
            let src  = ticket.src.clone();
            let dest = ticket.dest.clone().unwrap_or_default();
            ticket.src  = dest;
            ticket.dest = Some(src);
            route_network_message(NetworkMessage::TicketResponse(ticket)).await;
            None
        }

        NetworkMessage::TicketResponse(ticket) => {
            if let Err(e) = service_ticket(&ticket) {
                log::error!("service_ticket error: {e}");
            }
            None
        }

        NetworkMessage::TicketTraceResponse(trace) => {
            if let Err(e) = service_trace(trace) {
                log::error!("service_trace error: {e}");
            }
            None
        }

        NetworkMessage::MessagesRequest(req) => {
            let messages = pull_network_messages(&req).unwrap_or_else(|e| {
                log::error!("pull_network_messages error: {e}");
                Vec::new()
            });
            Some(NetworkMessage::MessagesResponse(NetworkMessagesResponse {
                messages,
                dest: req.isrc.clone(),
                ..Default::default()
            }))
        }

        _ => {
            log::warn!("unknown network message type encountered");
            None
        }
    }
    }) // end Box::pin
}

// ── Scheduled background functions ───────────────────────────────────────────

/// Replay pending network messages stored without a specific destination.
///
/// Mirrors Python's `@scheduled replay()`.
pub async fn replay() {
    match pop_network_messages(&[("dest", "$eq:null")]) {
        Ok(messages) => {
            for message in messages {
                tokio::spawn(async move {
                    route_network_message(message).await;
                });
            }
        }
        Err(e) => log::error!("replay error: {e}"),
    }
}

/// Poll all peers with polling enabled for pending messages.
///
/// Mirrors Python's `@scheduled polling()`.
pub async fn polling() {
    let peers =
        match open_peers().and_then(|p| p.find(&[("polling", "true"), ("url", "$!eq:null")])) {
            Ok(peers) => peers,
            Err(e) => {
                log::error!("polling: failed to get peers: {e}");
                return;
            }
        };

    for peer_obj in peers {
        let peer = peer_obj.object.clone();
        tokio::spawn(async move {
            let url = match peer.url {
                Some(u) => u,
                None    => return,
            };
            let client = AgentClient::with_credentials(
                url.clone(),
                config().key(),
                config().agtuuid.clone(),
            );
            match client
                .send_network_message(NetworkMessage::MessagesRequest(
                    NetworkMessagesRequest::default(),
                ))
                .await
            {
                Ok(NetworkMessage::MessagesResponse(resp)) => {
                    for msg in resp.messages {
                        tokio::spawn(async move {
                            route_network_message(msg).await;
                        });
                    }
                }
                Ok(NetworkMessage::Acknowledgement(ack)) => {
                    if let Some(ref err) = ack.error {
                        log::error!("poll acknowledgement error: {err}");
                    }
                }
                Ok(_) => {}
                Err(e) => log::error!("poll error for {url}: {e}"),
            }
        });
    }
}

/// Age routes and advertise current routes to all known peers.
///
/// Mirrors Python's `@scheduled advertizing()`.
pub async fn advertizing() {
    if let Err(e) = age_routes(1) {
        log::error!("age_routes error: {e}");
    }

    let peers = match open_peers().and_then(|p| p.find(&[])) {
        Ok(peers) => peers,
        Err(e) => {
            log::error!("advertizing: failed to get peers: {e}");
            return;
        }
    };

    for peer_obj in peers {
        let peer = peer_obj.object.clone();
        tokio::spawn(async move {
            let agtuuid = match peer.agtuuid {
                Some(ref id) => id.clone(),
                None         => return,
            };
            match create_route_advertisement() {
                Ok(mut adv) => {
                    adv.dest = Some(agtuuid);
                    route_network_message(NetworkMessage::Advertisement(adv)).await;
                }
                Err(e) => log::error!("create_route_advertisement error: {e}"),
            }
        });
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn extract_header_hex(req: &HttpRequest, name: &str) -> ActixResult<Vec<u8>> {
    let value = req
        .headers()
        .get(name)
        .ok_or_else(|| actix_web::error::ErrorBadRequest(format!("missing header: {name}")))?
        .to_str()
        .map_err(|e| actix_web::error::ErrorBadRequest(e.to_string()))?
        .to_string();
    hex::decode(value).map_err(|e| actix_web::error::ErrorBadRequest(e.to_string()))
}

fn config_to_json() -> Value {
    let c = config();
    json!({
        "agtuuid":             c.agtuuid,
        "workers":             c.workers,
        "socket_host":         c.socket_host,
        "socket_port":         c.socket_port,
        "secret_digest":       c.secret_digest,
        "client_control_url":  c.client_control_url,
        "log_level_api":       c.log_level_api.to_string(),
        "log_level_app":       c.log_level_app.to_string(),
        "peer_timeout_secs":   c.peer_timeout_secs,
        "peer_refresh_secs":   c.peer_refresh_secs,
        "max_weight":          c.max_weight,
        "ticket_timeout_secs": c.ticket_timeout_secs,
        "message_timeout_secs": c.message_timeout_secs,
    })
}

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

fn src_of(msg: &NetworkMessage) -> String {
    match msg {
        NetworkMessage::Ping(m)                => m.src.clone(),
        NetworkMessage::MessagesRequest(m)     => m.src.clone(),
        NetworkMessage::MessagesResponse(m)    => m.src.clone(),
        NetworkMessage::Acknowledgement(m)     => m.src.clone(),
        NetworkMessage::Advertisement(m)       => m.src.clone(),
        NetworkMessage::TicketTraceResponse(m) => m.src.clone(),
        NetworkMessage::TicketRequest(m)       => m.src.clone(),
        NetworkMessage::TicketResponse(m)      => m.src.clone(),
    }
}

fn isrc_of(msg: &NetworkMessage) -> Option<String> {
    match msg {
        NetworkMessage::Ping(m)                => m.isrc.clone(),
        NetworkMessage::MessagesRequest(m)     => m.isrc.clone(),
        NetworkMessage::MessagesResponse(m)    => m.isrc.clone(),
        NetworkMessage::Acknowledgement(m)     => m.isrc.clone(),
        NetworkMessage::Advertisement(m)       => m.isrc.clone(),
        NetworkMessage::TicketTraceResponse(m) => m.isrc.clone(),
        NetworkMessage::TicketRequest(m)       => m.isrc.clone(),
        NetworkMessage::TicketResponse(m)      => m.isrc.clone(),
    }
}

fn set_dest(msg: &mut NetworkMessage, dest: String) {
    let d = Some(dest);
    match msg {
        NetworkMessage::Ping(m)                => m.dest = d,
        NetworkMessage::MessagesRequest(m)     => m.dest = d,
        NetworkMessage::MessagesResponse(m)    => m.dest = d,
        NetworkMessage::Acknowledgement(m)     => m.dest = d,
        NetworkMessage::Advertisement(m)       => m.dest = d,
        NetworkMessage::TicketTraceResponse(m) => m.dest = d,
        NetworkMessage::TicketRequest(m)       => m.dest = d,
        NetworkMessage::TicketResponse(m)      => m.dest = d,
    }
}

fn msg_type_of(msg: &NetworkMessage) -> &'static str {
    msg.message_type()
}