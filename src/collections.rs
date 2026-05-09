//! Shared collection singletons for messaging, ticketing, and peering.
//!
//! Each public opener returns a cheap clone of a process-wide singleton
//! `Collection<T>`.  Because `Collection` wraps an `Arc<Mutex<Connection>>`
//! internally, all clones share the same SQLite connection and therefore the
//! same underlying data — whether the database is file-backed or in-memory.
//!
//! In-memory collections use a `file:?mode=memory` URI so that a single
//! named connection is created once and reused for the lifetime of the
//! process.  File-backed collections (peers, kvstore) open their usual
//! `{name}.sqlite` path.

use std::sync::OnceLock;

use anyhow::Result;

use crate::dao::collection::Collection;
use crate::dao::kvstore::KeyValuePair;
use crate::models::control::ControlFormTicket;
use crate::models::network::{NetworkMessage, TicketTraceResponse};
use crate::models::routing::{Peer, Route};

// ── In-memory connection URIs ─────────────────────────────────────────────────

const CONN_MESSAGES: &str = "file:stembot_messages?mode=memory";
const CONN_TICKETS:  &str = "file:stembot_tickets?mode=memory";
const CONN_TRACES:   &str = "file:stembot_traces?mode=memory";
const CONN_ROUTES:   &str = "file:stembot_routes?mode=memory";

// ── File-backed path helper ───────────────────────────────────────────────────

/// Returns the path for a file-backed SQLite database.
///
/// When built with the `STEMBOT_DATA_DIR` environment variable set, that
/// directory is used as the prefix (e.g. `/var/agt/peers.sqlite`).  Otherwise
/// the file is created in the current working directory.
fn db_path(name: &str) -> String {
    const DATA_DIR: &str = env!("STEMBOT_DATA_DIR");
    if DATA_DIR.is_empty() {
        format!("{}.sqlite", name)
    } else {
        format!("{}/{}.sqlite", DATA_DIR, name)
    }
}

// ── Singletons ────────────────────────────────────────────────────────────────

static MESSAGES: OnceLock<Collection<NetworkMessage>>      = OnceLock::new();
static TICKETS:  OnceLock<Collection<ControlFormTicket>>   = OnceLock::new();
static TRACES:   OnceLock<Collection<TicketTraceResponse>> = OnceLock::new();
static PEERS:    OnceLock<Collection<Peer>>                = OnceLock::new();
static ROUTES:   OnceLock<Collection<Route>>               = OnceLock::new();
static KVSTORE:  OnceLock<Collection<KeyValuePair>>        = OnceLock::new();

// ── Collection openers ────────────────────────────────────────────────────────

/// Open (or return the cached singleton for) the in-memory `messages` collection.
pub fn open_messages() -> Result<Collection<NetworkMessage>> {
    Ok(MESSAGES.get_or_init(|| {
        let c = Collection::new("messages", Some(CONN_MESSAGES))
            .expect("failed to open messages collection");
        c.create_attribute("dest",      "/dest").ok();
        c.create_attribute("timestamp", "/timestamp").ok();
        c
    }).clone())
}

/// Open (or return the cached singleton for) the in-memory `tickets` collection.
pub fn open_tickets() -> Result<Collection<ControlFormTicket>> {
    Ok(TICKETS.get_or_init(|| {
        let c = Collection::new("tickets", Some(CONN_TICKETS))
            .expect("failed to open tickets collection");
        c.create_attribute("tckuuid",     "/tckuuid").ok();
        c.create_attribute("create_time", "/create_time").ok();
        c
    }).clone())
}

/// Open (or return the cached singleton for) the in-memory `traces` collection.
pub fn open_traces() -> Result<Collection<TicketTraceResponse>> {
    Ok(TRACES.get_or_init(|| {
        let c = Collection::new("traces", Some(CONN_TRACES))
            .expect("failed to open traces collection");
        c.create_attribute("tckuuid",             "/tckuuid").ok();
        c.create_attribute("hop_time",            "/hop_time").ok();
        c.create_attribute("network_ticket_type", "/network_ticket_type").ok();
        c
    }).clone())
}

/// Open (or return the cached singleton for) the `peers` collection.
pub fn open_peers() -> Result<Collection<Peer>> {
    Ok(PEERS.get_or_init(|| {
        let c = Collection::new("peers", Some(db_path("peers").as_ref()))
            .expect("failed to open peers collection");
        c.create_attribute("agtuuid", "/agtuuid").ok();
        c.create_attribute("polling", "/polling").ok();
        c.create_attribute("url",     "/url").ok();
        c
    }).clone())
}

/// Open (or return the cached singleton for) the in-memory `routes` collection.
pub fn open_routes() -> Result<Collection<Route>> {
    Ok(ROUTES.get_or_init(|| {
        let c = Collection::new("routes", Some(CONN_ROUTES))
            .expect("failed to open routes collection");
        c.create_attribute("agtuuid", "/agtuuid").ok();
        c.create_attribute("gtwuuid", "/gtwuuid").ok();
        c.create_attribute("weight",  "/weight").ok();
        c
    }).clone())
}

/// Open (or return the cached singleton for) the `kvstore` collection.
pub fn open_kvstore() -> Result<Collection<KeyValuePair>> {
    Ok(KVSTORE.get_or_init(|| {
        let c = Collection::new("kvstore", Some(db_path("kvstore").as_ref()))
            .expect("failed to open kvstore collection");
        c.create_attribute("name", "/name").ok();
        c
    }).clone())
}

/// Vacuum all collections to reclaim space from deleted records.
pub fn vacuum_collections() -> Result<()> {
    open_messages()?.document.vacuum()?;
    open_tickets()?.document.vacuum()?;
    open_traces()?.document.vacuum()?;
    open_peers()?.document.vacuum()?;
    open_routes()?.document.vacuum()?;
    open_kvstore()?.document.vacuum()?;
    Ok(())
}