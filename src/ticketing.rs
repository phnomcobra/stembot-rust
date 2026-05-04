//! Ticket management for routed control forms and inter-agent communication.
//!
//! Mirrors Python's `stembot/ticketing.py`.
//!
//! Manages the lifecycle of control form tickets routed through the network:
//! - `read_ticket` — look up a ticket and populate hops if tracing is enabled
//! - `close_ticket` — delete a ticket by UUID
//! - `service_ticket` — record the response form and service time for a ticket
//! - `service_trace` — upsert a hop trace into the traces collection
//! - `dedup_trace` — deduplicate trace messages to prevent infinite loops
//! - `expire_tickets` — remove stale tickets and traces

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::collections::{open_tickets, open_traces};
use crate::enums::ControlFormType;
use crate::models::control::ControlFormTicket;
use crate::models::network::{NetworkTicket, TicketTraceResponse};
use crate::config::config;

fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Retrieve a ticket by UUID, populating hops if tracing is enabled.
///
/// Returns `None` if no ticket with the given UUID exists.
/// Sets the returned ticket's `form_type` to `ReadTicket`.
///
/// Mirrors `read_ticket(control_form_ticket)`.
pub fn read_ticket(control_form_ticket: &ControlFormTicket) -> Result<Option<ControlFormTicket>> {
    let tickets = open_tickets()?;
    let traces  = open_traces()?;

    for mut ticket in tickets.find(&[("tckuuid", control_form_ticket.tckuuid.as_str())])? {
        if ticket.object.tracing {
            ticket.object.hops = traces
                .find(&[("tckuuid", ticket.object.tckuuid.as_str())])?
                .into_iter()
                .map(|t| t.object.hop())
                .collect();
        }
        ticket.object.form_type = ControlFormType::ReadTicket;
        return Ok(Some(ticket.object));
    }
    Ok(None)
}

/// Delete a ticket by UUID from the in-memory ticket collection.
///
/// Mirrors `close_ticket(control_form_ticket)`.
pub fn close_ticket(control_form_ticket: &ControlFormTicket) -> Result<()> {
    open_tickets()?.pop(&[("tckuuid", control_form_ticket.tckuuid.as_str())])?;
    Ok(())
}

/// Update a ticket with the serviced control form and service time.
///
/// Mirrors `service_ticket(network_ticket)`.
pub fn service_ticket(network_ticket: &NetworkTicket) -> Result<()> {
    let tickets = open_tickets()?;
    for mut ticket in tickets.find(&[("tckuuid", network_ticket.tckuuid.as_str())])? {
        ticket.object.form         = network_ticket.form.clone();
        ticket.object.service_time = Some(unix_now());
        ticket.commit()?;
    }
    Ok(())
}

/// Add hop information to a ticket's trace for route tracking.
///
/// Mirrors `service_trace(ticket_trace)`.
pub fn service_trace(ticket_trace: TicketTraceResponse) -> Result<()> {
    open_traces()?.upsert_object(ticket_trace)?;
    Ok(())
}

/// Deduplicate trace messages for a ticket to prevent infinite loops.
///
/// For tickets with tracing enabled:
/// - If a trace for this ticket and message type already exists, updates its
///   `hop_time` and returns `None` (duplicate).
/// - Otherwise creates a new trace entry and returns it.
///
/// `ticket_type` is the wire-format type string of the wrapping ticket variant
/// (`"ticket_request"` or `"ticket_response"`).
///
/// Mirrors `dedup_trace(network_ticket)`.
pub fn dedup_trace(
    network_ticket: &NetworkTicket,
    ticket_type: &str,
) -> Result<Option<TicketTraceResponse>> {
    if !network_ticket.tracing {
        return Ok(None);
    }

    let traces = open_traces()?;

    let matches = traces.find(&[
        ("tckuuid",             network_ticket.tckuuid.as_str()),
        ("network_ticket_type", ticket_type),
    ])?;

    if !matches.is_empty() {
        // Duplicate — refresh the hop_time to keep it alive
        for mut trace in matches {
            trace.object.hop_time = unix_now();
            trace.commit()?;
        }
        Ok(None)
    } else {
        // First time — create and return a new trace
        let dest = if ticket_type == "ticket_request" {
            Some(network_ticket.src.clone())
        } else {
            network_ticket.dest.clone()
        };
        let trace = TicketTraceResponse {
            src:                 config().agtuuid.clone(),
            dest,
            tckuuid:             network_ticket.tckuuid.clone(),
            network_ticket_type: ticket_type.to_string(),
            hop_time:            unix_now(),
            ..TicketTraceResponse::default()
        };
        traces.upsert_object(trace.clone())?;
        Ok(Some(trace))
    }
}

/// Remove tickets and traces that have exceeded the configured timeout period.
///
/// Mirrors the `@scheduled worker()` in Python.
pub fn expire_tickets() -> Result<()> {
    let cutoff = unix_now() - config().ticket_timeout_secs as f64;
    let cutoff_str = format!("$lt:{}", cutoff);

    for obj in open_tickets()?.pop(&[("create_time", cutoff_str.as_str())])? {
        log::warn!(
            "Expiring ticket {}:{}",
            obj.object.form_type,
            obj.object.tckuuid
        );
    }

    for obj in open_traces()?.pop(&[("hop_time", cutoff_str.as_str())])? {
        log::debug!("Expiring trace {}", obj.object.tckuuid);
    }

    Ok(())
}
