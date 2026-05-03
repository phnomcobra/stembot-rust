//! Shared in-memory collection handles for messaging, ticketing, and peering.
//!
//! Each collection uses a named shared-cache in-memory SQLite database so that
//! all opened handles (across modules) see the same underlying data.
//!
//! Attribute indexes are created (idempotently) on every open so callers never
//! need to run separate setup.

use anyhow::Result;

use crate::dao::collection::Collection;
use crate::dao::kvstore::KeyValuePair;
use crate::models::control::ControlFormTicket;
use crate::models::network::{NetworkMessageVariant, TicketTraceResponse};
use crate::models::routing::{Peer, Route};

// ── Shared connection URIs ────────────────────────────────────────────────────

const CONN_MESSAGES: &str = "file:stembot_messages?mode=memory&cache=shared";
const CONN_TICKETS:  &str = "file:stembot_tickets?mode=memory&cache=shared";
const CONN_TRACES:   &str = "file:stembot_traces?mode=memory&cache=shared";
const CONN_ROUTES:   &str = "file:stembot_routes?mode=memory&cache=shared";

// ── Collection openers ────────────────────────────────────────────────────────

/// Open the in-memory `messages` collection.
/// Indexed by: `dest`, `timestamp`.
pub fn open_messages() -> Result<Collection<NetworkMessageVariant>> {
    let col = Collection::new("messages", Some(CONN_MESSAGES))?;
    col.create_attribute("dest",      "/dest")?;
    col.create_attribute("timestamp", "/timestamp")?;
    Ok(col)
}

/// Open the in-memory `tickets` collection.
/// Indexed by: `tckuuid`, `create_time`.
pub fn open_tickets() -> Result<Collection<ControlFormTicket>> {
    let col = Collection::new("tickets", Some(CONN_TICKETS))?;
    col.create_attribute("tckuuid",     "/tckuuid")?;
    col.create_attribute("create_time", "/create_time")?;
    Ok(col)
}

/// Open the in-memory `traces` collection.
/// Indexed by: `tckuuid`, `hop_time`, `network_ticket_type`.
pub fn open_traces() -> Result<Collection<TicketTraceResponse>> {
    let col = Collection::new("traces", Some(CONN_TRACES))?;
    col.create_attribute("tckuuid",             "/tckuuid")?;
    col.create_attribute("hop_time",            "/hop_time")?;
    col.create_attribute("network_ticket_type", "/network_ticket_type")?;
    Ok(col)
}

/// Open the `peers` collection.
/// Indexed by: `agtuuid`, `polling`, `url`.
pub fn open_peers() -> Result<Collection<Peer>> {
    let col = Collection::new("peers", None)?;
    col.create_attribute("agtuuid", "/agtuuid")?;
    col.create_attribute("polling", "/polling")?;
    col.create_attribute("url",     "/url")?;
    Ok(col)
}

/// Open the in-memory `routes` collection.
/// Indexed by: `agtuuid`, `gtwuuid`, `weight`.
pub fn open_routes() -> Result<Collection<Route>> {
    let col = Collection::new("routes", Some(CONN_ROUTES))?;
    col.create_attribute("agtuuid", "/agtuuid")?;
    col.create_attribute("gtwuuid", "/gtwuuid")?;
    col.create_attribute("weight",  "/weight")?;
    Ok(col)
}

/// Open the `kvstore` collection for key-value pairs.
/// Indexed by: `name`.
pub fn open_kvstore() -> Result<Collection<KeyValuePair>> {
    let col = Collection::new("kvstore", None)?;
    col.create_attribute("name", "/name")?;
    Ok(col)
}
