//! Peer and route management for network topology discovery and maintenance.
//!
//! Mirrors Python's `stembot/peering.py`.
//!
//! Handles the complete lifecycle of peers and routes:
//! - Peer discovery, creation, and lifecycle management with TTL and polling
//! - Route creation and aging to maintain optimal paths through the network
//! - Route advertisement processing to discover new network paths
//! - Network topology cleanup and pruning of expired entries

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::collections::{open_peers, open_routes};
use crate::models::network::Advertisement;
use crate::models::routing::{Peer, Route};
use crate::config::config;

fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

// ── Peer management ───────────────────────────────────────────────────────────

/// Touch a peer to refresh its timestamps, or create it if it doesn't exist.
///
/// Updates an existing peer's timers only when it has no URL and its refresh
/// time has passed; otherwise creates a new transient entry.
///
/// Mirrors `touch_peer(agtuuid)`.
pub fn touch_peer(agtuuid: &str) -> Result<()> {
    let peers = open_peers()?.find(&[("agtuuid", agtuuid)])?;

    if peers.is_empty() {
        create_peer(agtuuid, None, Some(config().peer_timeout_secs), false)?
    } else {
        let peer = &peers[0];
        if peer.object.url.is_none()
            && peer.object.refresh_time.is_some_and(|rt| rt < unix_now())
        {
            create_peer(agtuuid, None, Some(config().peer_timeout_secs), false)?
        }
    }
    Ok(())
}

/// Delete a peer from the in-memory peer collection.
///
/// Mirrors `delete_peer(agtuuid)`.
pub fn delete_peer(agtuuid: &str) -> Result<()> {
    open_peers()?.pop(&[("agtuuid", agtuuid)])?;
    Ok(())
}

/// Delete all peers from the in-memory peer collection.
///
/// Mirrors `delete_peers()`.
pub fn delete_peers() -> Result<()> {
    open_peers()?.pop(&[])?;
    Ok(())
}

/// Create or update a peer in the in-memory peer collection.
///
/// If a peer with the given `agtuuid` already exists it is updated in place;
/// otherwise a new object is allocated.  TTL sets `destroy_time` and
/// `refresh_time` relative to the current time and configured intervals.
///
/// Mirrors `create_peer(agtuuid, url, ttl, polling)`.
pub fn create_peer(
    agtuuid:  &str,
    url:      Option<String>,
    ttl:      Option<u32>,
    polling:  bool,
) -> Result<()> {
    let peers = open_peers()?;

    let mut matches = peers.find(&[("agtuuid", agtuuid)])?;

    let mut peer = if matches.len() == 1 {
        matches.remove(0)
    } else {
        peers.get_object(None)?
    };

    peer.object.agtuuid  = Some(agtuuid.to_string());
    peer.object.url      = url;
    peer.object.polling  = polling;

    if let Some(ttl_secs) = ttl {
        let now = unix_now();
        peer.object.destroy_time = Some(now + ttl_secs as f64);
        peer.object.refresh_time = Some(now + config().peer_refresh_secs as f64);
    } else {
        peer.object.destroy_time = None;
        peer.object.refresh_time = None;
    }

    peer.commit()?;
    Ok(())
}

// ── Route management ──────────────────────────────────────────────────────────

/// Delete a specific route from the in-memory route collection.
///
/// Mirrors `delete_route(agtuuid, gtwuuid)`.
pub fn delete_route(agtuuid: &str, gtwuuid: &str) -> Result<()> {
    for obj in open_routes()?.find(&[("agtuuid", agtuuid), ("gtwuuid", gtwuuid)])? {
        obj.destroy()?;
    }
    Ok(())
}

/// Increase the weight of all routes and remove those exceeding max weight.
///
/// Implements route aging: routes with weight > `max_weight` are deleted;
/// others have their weight incremented by `v`.
///
/// Mirrors `age_routes(v)`.
pub fn age_routes(v: i64) -> Result<()> {
    let max_weight = config().max_weight as i64;
    for mut obj in open_routes()?.find(&[])? {
        if obj.object.weight > max_weight {
            obj.destroy()?;
        } else {
            obj.object.weight += v;
            obj.commit()?;
        }
    }
    Ok(())
}

/// Create or update a route (agtuuid → via gtwuuid) with the given weight.
///
/// - If multiple duplicate entries exist they are cleared and a single route is
///   (re)created.
/// - If one entry exists with a higher weight, its weight is lowered.
/// - If no entry exists one is created.
///
/// Mirrors `create_route(agtuuid, gtwuuid, weight)`.
pub fn create_route(agtuuid: &str, gtwuuid: &str, weight: i64) -> Result<()> {
    let routes = open_routes()?;
    let matches = routes.find(&[("agtuuid", agtuuid), ("gtwuuid", gtwuuid)])?;

    match matches.len() {
        0 => {
            routes.build_object(Route {
                agtuuid: agtuuid.to_string(),
                gtwuuid: gtwuuid.to_string(),
                weight,
                ..Default::default()
            })?;
        }
        1 => {
            let mut route = matches.into_iter().next().unwrap();
            if route.object.weight > weight {
                route.object.weight = weight;
                route.commit()?;
            }
        }
        _ => {
            // Duplicates — remove all and recreate a single entry
            for route in matches {
                route.destroy()?;
            }
            routes.build_object(Route {
                agtuuid: agtuuid.to_string(),
                gtwuuid: gtwuuid.to_string(),
                weight,
                ..Default::default()
            })?;
        }
    }
    Ok(())
}

// ── Route advertisements ──────────────────────────────────────────────────────

/// Process a route advertisement from a peer and create local routes.
///
/// Ignores routes to self and already-known peers.  Increments advertised
/// weights by 1 to account for the additional hop through this agent.
/// Runs `prune()` afterwards to clean up stale data.
///
/// Mirrors `process_route_advertisement(advertisement)`.
pub fn process_route_advertisement(advertisement: &Advertisement) -> Result<()> {
    let peers  = open_peers()?;

    let mut ignored: Vec<String> = vec![config().agtuuid.clone()];
    for obj in peers.find(&[])? {
        if let Some(ref a) = obj.object.agtuuid {
            ignored.push(a.clone());
        }
    }

    for route in advertisement.routes.iter().filter(|r| !ignored.contains(&r.agtuuid)) {
        create_route(&route.agtuuid, &advertisement.agtuuid, route.weight + 1)?;
    }

    prune()?;
    Ok(())
}

/// Build an advertisement containing all known routes and directly reachable peers.
///
/// Sets each route's `gtwuuid` to this agent's UUID so the recipient can route
/// back through us.  Runs `prune()` first to ensure stale data is removed.
///
/// Mirrors `create_route_advertisement()`.
pub fn create_route_advertisement() -> Result<Advertisement> {
    prune()?;

    let routes = open_routes()?;
    let peers  = open_peers()?;

    let mut advertisement = Advertisement {
        agtuuid: config().agtuuid.clone(),
        ..Default::default()
    };

    // Advertise all known routes (via self as the next hop)
    for obj in routes.find(&[])? {
        let mut route = obj.object.clone();
        route.gtwuuid = config().agtuuid.clone();
        advertisement.routes.push(route);
    }

    // Also advertise direct peers as zero-weight routes
    for obj in peers.find(&[("agtuuid", "$!eq:null")])? {
        if let Some(ref agtuuid) = obj.object.agtuuid {
            advertisement.routes.push(Route {
                agtuuid: agtuuid.clone(),
                gtwuuid: config().agtuuid.clone(),
                weight:  0,
                ..Default::default()
            });
        }
    }

    Ok(advertisement)
}

// ── Peer/route queries ────────────────────────────────────────────────────────

/// Return all peers from the in-memory peer collection.
///
/// Mirrors `get_peers()`.
pub fn get_peers() -> Result<Vec<Peer>> {
    Ok(open_peers()?
        .find(&[])?
        .into_iter()
        .map(|o| o.object)
        .collect())
}

/// Return all routes from the in-memory route collection.
///
/// Mirrors `get_routes()`.
pub fn get_routes() -> Result<Vec<Route>> {
    Ok(open_routes()?
        .find(&[])?
        .into_iter()
        .map(|o| o.object)
        .collect())
}

// ── Pruning ───────────────────────────────────────────────────────────────────

/// Remove expired peers and routes with invalid gateways.
///
/// Deletes peers whose `destroy_time` has passed, then removes routes that
/// point to non-existent gateways, routes whose destination is directly reachable
/// as a peer, and routes to self.
///
/// Mirrors `prune()`.
pub fn prune() -> Result<()> {
    let now = unix_now();

    let routes = open_routes()?;
    let peers  = open_peers()?;

    let mut peer_agtuuids: Vec<String> = Vec::new();

    for peer in peers.find(&[])? {
        if peer.object.destroy_time.is_some_and(|dt| dt < now) {
            peer.destroy()?;
            continue;
        }
        if let Some(ref a) = peer.object.agtuuid {
            peer_agtuuids.push(a.clone());
        }
    }

    for route in routes.find(&[])? {
        let gtwuuid_known = peer_agtuuids.contains(&route.object.gtwuuid);
        let dest_is_peer  = !peers
            .find(&[("agtuuid", route.object.agtuuid.as_str())])?
            .is_empty();
        let is_self = route.object.agtuuid == config().agtuuid;

        if !gtwuuid_known || dest_is_peer || is_self {
            route.destroy()?;
        }
    }

    Ok(())
}
