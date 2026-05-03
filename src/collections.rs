//! Shared in-memory collection handles for messaging, ticketing, and peering.
//!
//! Each collection uses a named shared-cache in-memory SQLite database so that
//! all opened handles (across modules) see the same underlying data.
//!
//! Attribute indexes are created once at first use via [`ensure_init`].

use std::sync::OnceLock;

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

// ── One-time attribute initialisation ────────────────────────────────────────

static INIT: OnceLock<()> = OnceLock::new();

/// Create all collection attributes exactly once per process.
///
/// Every opener calls this before returning its handle.  After the first call
/// the `OnceLock` short-circuits with a single atomic load.
fn ensure_init() {
    INIT.get_or_init(|| {
        log::info!("Initializing collection attributes");
        if let Ok(c) = Collection::<NetworkMessageVariant>::new("messages", None) {
            c.create_attribute("dest",      "/dest").ok();
            c.create_attribute("timestamp", "/timestamp").ok();
        }
        if let Ok(c) = Collection::<ControlFormTicket>::new("tickets", None) {
            c.create_attribute("tckuuid",     "/tckuuid").ok();
            c.create_attribute("create_time", "/create_time").ok();
        }
        if let Ok(c) = Collection::<TicketTraceResponse>::new("traces", None) {
            c.create_attribute("tckuuid",             "/tckuuid").ok();
            c.create_attribute("hop_time",            "/hop_time").ok();
            c.create_attribute("network_ticket_type", "/network_ticket_type").ok();
        }
        if let Ok(c) = Collection::<Peer>::new("peers", None) {
            c.create_attribute("agtuuid", "/agtuuid").ok();
            c.create_attribute("polling", "/polling").ok();
            c.create_attribute("url",     "/url").ok();
        }
        if let Ok(c) = Collection::<Route>::new("routes", None) {
            c.create_attribute("agtuuid", "/agtuuid").ok();
            c.create_attribute("gtwuuid", "/gtwuuid").ok();
            c.create_attribute("weight",  "/weight").ok();
        }
        if let Ok(c) = Collection::<KeyValuePair>::new("kvstore", None) {
            c.create_attribute("name", "/name").ok();
        }
    });
}

// ── Collection openers ────────────────────────────────────────────────────────

/// Open the in-memory `messages` collection.
pub fn open_messages() -> Result<Collection<NetworkMessageVariant>> {
    ensure_init();
    Collection::new("messages", None)
}

/// Open the in-memory `tickets` collection.
pub fn open_tickets() -> Result<Collection<ControlFormTicket>> {
    ensure_init();
    Collection::new("tickets", None)
}

/// Open the in-memory `traces` collection.
pub fn open_traces() -> Result<Collection<TicketTraceResponse>> {
    ensure_init();
    Collection::new("traces", None)
}

/// Open the `peers` collection.
pub fn open_peers() -> Result<Collection<Peer>> {
    ensure_init();
    Collection::new("peers", None)
}

/// Open the in-memory `routes` collection.
pub fn open_routes() -> Result<Collection<Route>> {
    ensure_init();
    Collection::new("routes", None)
}

/// Open the `kvstore` collection for key-value pairs.
pub fn open_kvstore() -> Result<Collection<KeyValuePair>> {
    ensure_init();
    Collection::new("kvstore", None)
}
