//! CLI control interface for agent management and network operations.
//!
//! Mirrors Python's `stembot/control.py`.

use std::process;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use chrono::{SecondsFormat, TimeZone, Utc};
use clap::{Parser, Subcommand};
use tokio::time::{sleep, Duration};

use stembot_rust::{
    executor::{
        agent::AgentClient,
        file::{load_file_to_form, write_file_from_form},
    },
    models::{
        config::Config,
        control::{
            Benchmark, CheckTicket, CloseTicket, CommandArg, ControlForm, ControlFormTicket,
            DeletePeers, DiscoverPeer, GetConfig, GetPeers, GetRoutes, LoadFile, SyncProcess,
            WriteFile,
        },
    },
};

const KB: usize = 1_024;
const MB: usize = 1_024 * 1_024;
const GB: usize = 1_024 * 1_024 * 1_024;

fn format_bytes(n: f64) -> String {
    if n < KB as f64       { format!("{n:.0} B") }
    else if n < MB as f64  { format!("{:.1} KB", n / KB as f64) }
    else if n < GB as f64  { format!("{:.1} MB", n / MB as f64) }
    else                   { format!("{:.1} GB", n / GB as f64) }
}

fn format_bandwidth(bps: f64) -> String {
    if bps < KB as f64      { format!("{bps:.0} B/s") }
    else if bps < MB as f64 { format!("{:.1} KB/s", bps / KB as f64) }
    else if bps < GB as f64 { format!("{:.1} MB/s", bps / MB as f64) }
    else                    { format!("{:.1} GB/s", bps / GB as f64) }
}

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[clap(name = "agt-control", about = "Agent control and network management")]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Discover and establish connection with a peer agent
    Discover {
        /// URL of the peer agent to discover (e.g., http://peer:8080/mpi)
        peer_url: String,
        /// Enable polling mode for continuous discovery updates
        #[clap(short = 'p', long)]
        polling: bool,
        /// Delay making discovery request for n seconds
        #[clap(short = 'd', long)]
        delay: Option<u64>,
        /// Time-to-live for the discovery in seconds
        #[clap(long)]
        ttl: Option<f64>,
    },
    /// Remove agents from the network
    Delete {
        /// Delete all agents
        #[clap(long = "all")]
        delete_all: bool,
        /// Delete a specific agent by UUID
        #[clap(long)]
        agtuuid: Option<String>,
    },
    /// Retrieve and display agent statistics
    Stat {
        /// UUID of the agent to query
        agtuuid: String,
        /// Timeout in seconds (default: 15)
        #[clap(short = 't', long, default_value = "15")]
        timeout: u64,
    },
    /// Benchmark agent file I/O performance across multiple file sizes
    Bench {
        /// UUID of the agent to benchmark
        agtuuid: String,
        /// Timeout in seconds per operation (default: 15)
        #[clap(short = 't', long, default_value = "15")]
        timeout: u64,
    },
    /// Transfer a file from source to destination
    Put {
        /// Source file path
        src_path: String,
        /// Destination file path
        dst_path: String,
        /// Timeout in seconds (default: 15)
        #[clap(short = 't', long, default_value = "15")]
        timeout: u64,
        /// Source agent UUID (reads from local filesystem if omitted)
        #[clap(short = 's', long)]
        src_agtuuid: Option<String>,
        /// Destination agent UUID (writes to local filesystem if omitted)
        #[clap(short = 'd', long)]
        dst_agtuuid: Option<String>,
    },
    /// Execute a command on a remote agent
    Run {
        /// UUID of the agent to run the command on
        agtuuid: String,
        /// Command to execute
        command: String,
        /// Timeout in seconds (default: 15)
        #[clap(short = 't', long, default_value = "15")]
        timeout: u64,
    },
}

// ── Ticket polling ────────────────────────────────────────────────────────────

/// Poll a ticket until serviced or timeout, then read and close it.
///
/// 1. Uses `CheckTicket` (lightweight) to poll for service completion.
/// 2. Once serviced, reads the full ticket via `read_ticket`.
/// 3. Closes the ticket via `CloseTicket`.
/// 4. Mirrors the Python polling pattern in `stembot/control.py`.
async fn poll_ticket(
    client: Arc<AgentClient>,
    ticket: ControlFormTicket,
    timeout_secs: u64,
) -> ControlFormTicket {
    let start = Instant::now();

    // Poll with CheckTicket until the ticket is serviced
    let mut check = CheckTicket {
        tckuuid:     ticket.tckuuid.clone(),
        create_time: Some(ticket.create_time),
        ..Default::default()
    };
    while start.elapsed().as_secs() < timeout_secs && check.service_time.is_none() {
        match client.send_control_form(ControlForm::CheckTicket(check.clone())).await {
            Ok(ControlForm::CheckTicket(c)) => check = c,
            Ok(_) => break,
            Err(e) => { eprintln!("poll error: {e}"); break; }
        }
        if check.service_time.is_none() {
            sleep(Duration::from_secs(1)).await;
        }
    }

    // Read the full ticket result
    let mut read = ticket.clone();
    read.form_type = "read_ticket".to_string();
    let result = match client.send_ticket(read.clone()).await {
        Ok(t)  => t,
        Err(e) => { eprintln!("read ticket error: {e}"); read }
    };

    // Close the ticket
    let close = ControlForm::CloseTicket(CloseTicket {
        tckuuid: ticket.tckuuid.clone(),
        ..Default::default()
    });
    if let Err(e) = client.send_control_form(close).await {
        eprintln!("close ticket error: {e}");
    }

    result
}

// ── Commands ──────────────────────────────────────────────────────────────────

async fn cmd_discover(
    client: Arc<AgentClient>,
    peer_url: String,
    polling: bool,
    delay: Option<u64>,
    ttl: Option<f64>,
) -> Result<()> {
    if let Some(d) = delay {
        println!("Waiting {d} seconds before discovery...");
        sleep(Duration::from_secs(d)).await;
    }

    println!("Discovering peer: {peer_url}");

    let result = client
        .send_control_form(ControlForm::DiscoverPeer(DiscoverPeer {
            url: peer_url.clone(),
            polling,
            ttl,
            ..Default::default()
        }))
        .await?;

    let (agtuuid, url, returned_ttl, returned_polling, error, objuuid, coluuid) =
        match &result {
            ControlForm::DiscoverPeer(f) => (
                f.agtuuid.clone(),
                f.url.clone(),
                f.ttl,
                f.polling,
                f.error.clone(),
                f.objuuid.clone(),
                f.coluuid.clone(),
            ),
            _ => (None, peer_url, ttl, polling, None, None, None),
        };

    println!();
    println!("{}", "=".repeat(70));
    println!("Peer Discovery Result");
    println!("{}", "=".repeat(70));

    if let Some(ref e) = error {
        println!();
        println!("X Error");
        println!("   {e}");
    } else {
        println!();
        println!("✓ Discovery Successful");
    }

    println!();
    println!("Discovery Details");
    println!("   Peer URL..................... {url}");
    if let Some(ref id) = agtuuid {
        println!("   Agent UUID................... {id}");
    } else {
        println!("   Agent UUID................... (not yet assigned)");
    }
    if let Some(t) = returned_ttl {
        println!("   TTL (Time-to-Live)........... {t} seconds");
    } else {
        println!("   TTL (Time-to-Live)........... (not set)");
    }
    let poll_str = if returned_polling { "Enabled" } else { "Disabled" };
    println!("   Polling Mode................. {poll_str}");

    println!();
    println!("Form Details");
    println!("   Type......................... {}", result.form_type());
    if let Some(ref id) = objuuid {
        println!("   Object UUID.................. {id}");
    }
    if let Some(ref id) = coluuid {
        println!("   Collection UUID.............. {id}");
    }

    println!();
    println!("{}", "=".repeat(70));
    println!();
    Ok(())
}

async fn cmd_delete(
    client: Arc<AgentClient>,
    delete_all: bool,
    agtuuid: Option<String>,
) -> Result<()> {
    if delete_all {
        println!("Deleting all agents...");
        let result = client
            .send_control_form(ControlForm::DeletePeers(DeletePeers::default()))
            .await?;
        if let ControlForm::DeletePeers(f) = result {
            if let Some(e) = f.error { eprintln!("{e}"); }
        }
    } else if let Some(id) = agtuuid {
        println!("Deleting agent: {id}");
        let result = client
            .send_control_form(ControlForm::DeletePeers(DeletePeers {
                agtuuids: Some(vec![id]),
                ..Default::default()
            }))
            .await?;
        if let ControlForm::DeletePeers(f) = result {
            if let Some(e) = f.error { eprintln!("{e}"); }
        }
    } else {
        eprintln!("Error: Use --all or --agtuuid <UUID>");
    }
    Ok(())
}

async fn cmd_stat(client: Arc<AgentClient>, agtuuid: String, timeout: u64) -> Result<()> {
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

/// Poll a `Benchmark` ticket for timing data only (no content read).
///
/// Returns the `CheckTicket` with `create_time` and `service_time` populated.
async fn bench_poll_timing(
    client: Arc<AgentClient>,
    ticket: ControlFormTicket,
    timeout_secs: u64,
) -> CheckTicket {
    let start = std::time::Instant::now();
    let mut check = CheckTicket {
        tckuuid:     ticket.tckuuid.clone(),
        create_time: Some(ticket.create_time),
        ..Default::default()
    };
    while start.elapsed().as_secs() < timeout_secs && check.service_time.is_none() {
        match client.send_control_form(ControlForm::CheckTicket(check.clone())).await {
            Ok(ControlForm::CheckTicket(c)) => check = c,
            Ok(_) => break,
            Err(e) => { eprintln!("poll error: {e}"); break; }
        }
        if check.service_time.is_none() {
            sleep(Duration::from_secs(1)).await;
        }
    }
    let close = ControlForm::CloseTicket(CloseTicket {
        tckuuid: ticket.tckuuid.clone(),
        ..Default::default()
    });
    if let Err(e) = client.send_control_form(close).await {
        eprintln!("close ticket error: {e}");
    }
    check
}

/// Send and poll a batch of Benchmark tickets for one direction.
///
/// `outbound = true`  → measures OUT throughput (client → agent, `outbound_size` set).
/// `outbound = false` → measures IN  throughput (agent → client, `inbound_size` set).
async fn bench_run_direction(
    client: Arc<AgentClient>,
    agtuuid: String,
    size: usize,
    concurrency: usize,
    timeout_secs: u64,
    outbound: bool,
) -> Vec<CheckTicket> {
    let tickets: Vec<ControlFormTicket> = (0..concurrency)
        .map(|_| {
            let form = if outbound {
                ControlForm::Benchmark(Benchmark {
                    outbound_size: Some(size as i64),
                    inbound_size:  None,
                    ..Default::default()
                })
            } else {
                ControlForm::Benchmark(Benchmark {
                    outbound_size: None,
                    inbound_size:  Some(size as i64),
                    ..Default::default()
                })
            };
            ControlFormTicket { dst: agtuuid.clone(), form, ..ControlFormTicket::default() }
        })
        .collect();

    // Send tickets concurrently
    let mut join_set = tokio::task::JoinSet::new();
    for ticket in tickets {
        let c = Arc::clone(&client);
        join_set.spawn(async move { c.send_ticket(ticket).await });
    }
    let mut sent: Vec<ControlFormTicket> = Vec::new();
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok(t))  => sent.push(t),
            Ok(Err(e)) => eprintln!("bench send error: {e}"),
            Err(e)     => eprintln!("join error: {e}"),
        }
    }

    // Poll tickets concurrently for timing
    let mut join_set = tokio::task::JoinSet::new();
    for ticket in sent {
        let c = Arc::clone(&client);
        join_set.spawn(async move { bench_poll_timing(c, ticket, timeout_secs).await });
    }
    let mut checks: Vec<CheckTicket> = Vec::new();
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(c)  => checks.push(c),
            Err(e) => eprintln!("join error: {e}"),
        }
    }
    checks
}

/// Send and poll a batch of Benchmark tickets with both directions active simultaneously.
async fn bench_run_combined(
    client: Arc<AgentClient>,
    agtuuid: String,
    size: usize,
    concurrency: usize,
    timeout_secs: u64,
) -> Vec<CheckTicket> {
    let tickets: Vec<ControlFormTicket> = (0..concurrency)
        .map(|_| {
            let form = ControlForm::Benchmark(Benchmark {
                outbound_size: Some(size as i64),
                inbound_size:  Some(size as i64),
                ..Default::default()
            });
            ControlFormTicket { dst: agtuuid.clone(), form, ..ControlFormTicket::default() }
        })
        .collect();

    // Send tickets concurrently
    let mut join_set = tokio::task::JoinSet::new();
    for ticket in tickets {
        let c = Arc::clone(&client);
        join_set.spawn(async move { c.send_ticket(ticket).await });
    }
    let mut sent: Vec<ControlFormTicket> = Vec::new();
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok(t))  => sent.push(t),
            Ok(Err(e)) => eprintln!("bench send error: {e}"),
            Err(e)     => eprintln!("join error: {e}"),
        }
    }

    // Poll tickets concurrently for timing
    let mut join_set = tokio::task::JoinSet::new();
    for ticket in sent {
        let c = Arc::clone(&client);
        join_set.spawn(async move { bench_poll_timing(c, ticket, timeout_secs).await });
    }
    let mut checks: Vec<CheckTicket> = Vec::new();
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(c)  => checks.push(c),
            Err(e) => eprintln!("join error: {e}"),
        }
    }
    checks
}

async fn bench_run(
    client: Arc<AgentClient>,
    agtuuid: String,
    size: usize,
    concurrency: usize,
    timeout_secs: u64,
) {
    let out_checks  = bench_run_direction(Arc::clone(&client), agtuuid.clone(), size, concurrency, timeout_secs, true).await;
    let in_checks   = bench_run_direction(Arc::clone(&client), agtuuid.clone(), size, concurrency, timeout_secs, false).await;
    let comb_checks = bench_run_combined(Arc::clone(&client),  agtuuid,         size, concurrency, timeout_secs).await;

    let count_ok    = |checks: &[CheckTicket]| checks.iter().filter(|c| c.service_time.is_some()).count();
    let max_elapsed = |checks: &[CheckTicket]| {
        checks.iter()
            .filter_map(|c| c.service_time.zip(c.create_time).map(|(s, ct)| s - ct))
            .fold(0.0f64, f64::max)
    };

    let out_ok   = count_ok(&out_checks);
    let in_ok    = count_ok(&in_checks);
    let comb_ok  = count_ok(&comb_checks);

    let out_elapsed  = max_elapsed(&out_checks);
    let in_elapsed   = max_elapsed(&in_checks);
    let comb_elapsed = max_elapsed(&comb_checks);

    let out_bw  = if out_elapsed  > 0.0 { (out_ok  * size) as f64 / out_elapsed  } else { 0.0 };
    let in_bw   = if in_elapsed   > 0.0 { (in_ok   * size) as f64 / in_elapsed   } else { 0.0 };
    // combined transfers size bytes each direction per ticket
    let comb_bw = if comb_elapsed > 0.0 { (comb_ok * size * 2) as f64 / comb_elapsed } else { 0.0 };

    println!(
        "   {:<10} {:<15} {:<14} {:<14} {}",
        format_bytes(size as f64),
        format!("{in_ok}:{out_ok}:{comb_ok}:{concurrency}"),
        format_bandwidth(in_bw),
        format_bandwidth(out_bw),
        format_bandwidth(comb_bw),
    );
}

async fn cmd_bench(
    client: Arc<AgentClient>,
    agtuuid: String,
    timeout: u64,
) -> Result<()> {
    println!();
    println!("{}", "=".repeat(76));
    println!("Benchmark Results for {agtuuid}");
    println!("{}", "=".repeat(76));
    println!();

    println!(
        "   {:.<10} {:.<15} {:.<14} {:.<14} {}",
        "Bytes/Op", "Success", "IN BW", "OUT BW", "Overall BW"
    );
    println!("{}", "-".repeat(76));

    let sizes:         Vec<usize> = (0..17).map(|x| (16 * KB) << x).collect();
    let concurrencies: Vec<usize> = (0..7).map(|x| 1 << x).collect();

    for size in &sizes {
        for concurrency in &concurrencies {
            if size * concurrency > 256 * MB {
                continue;
            }
            bench_run(Arc::clone(&client), agtuuid.clone(), *size, *concurrency, timeout).await;
        }
    }

    println!("{}", "-".repeat(76));
    println!();
    println!("{}", "=".repeat(76));
    println!();
    Ok(())
}

async fn cmd_put(
    client: Arc<AgentClient>,
    src_path: String,
    dst_path: String,
    timeout: u64,
    src_agtuuid: Option<String>,
    dst_agtuuid: Option<String>,
) -> Result<()> {
    let read_elapsed;
    let mut write_elapsed = 0.0f64;
    let mut read_error:  Option<String> = None;
    let mut write_error: Option<String> = None;

    // Load file from source
    let mut load_form = LoadFile { path: src_path.clone(), ..Default::default() };

    if let Some(ref src_id) = src_agtuuid {
        println!("Reading from {src_id}:{src_path}...");
        let read_start = Instant::now();
        let ticket = client
            .send_ticket(ControlFormTicket {
                dst: src_id.clone(),
                form: ControlForm::LoadFile(load_form.clone()),
                ..ControlFormTicket::default()
            })
            .await?;
        let ticket = poll_ticket(Arc::clone(&client), ticket, timeout * 2).await;
        read_elapsed = read_start.elapsed().as_secs_f64();

        if ticket.service_time.is_none() {
            let e = "Load ticket never serviced!".to_string();
            eprintln!("{e}");
            read_error = Some(e);
        }
        if let Some(ref e) = ticket.error {
            eprintln!("{e}");
            read_error.get_or_insert(e.clone());
        }
        if let ControlForm::LoadFile(ref f) = ticket.form {
            if let Some(ref e) = f.error {
                eprintln!("{e}");
                read_error.get_or_insert(e.clone());
            }
            load_form = f.clone();
        }
    } else {
        println!("Reading from local:{src_path}...");
        let read_start = Instant::now();
        load_form = load_file_to_form(load_form);
        read_elapsed = read_start.elapsed().as_secs_f64();
        if let Some(ref e) = load_form.error {
            eprintln!("{e}");
            read_error = Some(e.clone());
        }
    }

    // Write file to destination
    if read_error.is_none() {
        let wf = WriteFile {
            b64zlib: load_form.b64zlib.clone().unwrap_or_default(),
            md5sum:  load_form.md5sum.clone(),
            size:    load_form.size,
            path:    dst_path.clone(),
            ..Default::default()
        };

        if let Some(ref dst_id) = dst_agtuuid {
            println!("Writing to {dst_id}:{dst_path}...");
            let write_start = Instant::now();
            let ticket = client
                .send_ticket(ControlFormTicket {
                    dst: dst_id.clone(),
                    form: ControlForm::WriteFile(wf),
                    ..ControlFormTicket::default()
                })
                .await?;
            let ticket = poll_ticket(Arc::clone(&client), ticket, timeout * 2).await;
            write_elapsed = write_start.elapsed().as_secs_f64();

            if ticket.service_time.is_none() {
                let e = "Write ticket never serviced!".to_string();
                eprintln!("{e}");
                write_error = Some(e);
            }
            if let Some(ref e) = ticket.error {
                eprintln!("{e}");
                write_error.get_or_insert(e.clone());
            }
            if let ControlForm::WriteFile(ref f) = ticket.form {
                if let Some(ref e) = f.error {
                    eprintln!("{e}");
                    write_error.get_or_insert(e.clone());
                }
            }
        } else {
            println!("Writing to local:{dst_path}...");
            let write_start = Instant::now();
            let written = write_file_from_form(wf);
            write_elapsed = write_start.elapsed().as_secs_f64();
            if let Some(ref e) = written.error {
                eprintln!("{e}");
                write_error = Some(e.clone());
            }
        }
    }

    // Pretty print results
    println!();
    println!("{}", "=".repeat(70));
    println!("File Transfer Result");
    println!("{}", "=".repeat(70));

    println!();
    println!("Transfer Details");
    let src_loc = src_agtuuid.as_deref()
        .map(|id| format!("{id}:{src_path}"))
        .unwrap_or_else(|| format!("local:{src_path}"));
    let dst_loc = dst_agtuuid.as_deref()
        .map(|id| format!("{id}:{dst_path}"))
        .unwrap_or_else(|| format!("local:{dst_path}"));
    println!("   Source................... {src_loc}");
    println!("   Destination.............. {dst_loc}");

    println!();
    println!("File Information");
    println!("   Size..................... {} bytes", load_form.size.unwrap_or(0));
    println!("   MD5 Checksum............. {}", load_form.md5sum.as_deref().unwrap_or("N/A"));

    println!();
    println!("Timing Information");
    println!("   Read Elapsed Time........ {read_elapsed:.3} seconds");
    println!("   Write Elapsed Time....... {write_elapsed:.3} seconds");
    println!("   Total Elapsed Time....... {:.3} seconds", read_elapsed + write_elapsed);

    if read_error.is_some() || write_error.is_some() {
        println!();
        println!("Errors Occurred");
        if let Some(ref e) = read_error  { println!("   Read Error............... {e}"); }
        if let Some(ref e) = write_error { println!("   Write Error.............. {e}"); }
    } else {
        println!();
        println!("✓ Transfer Complete");
    }

    println!();
    println!("{}", "=".repeat(70));
    println!();
    Ok(())
}

async fn cmd_run(
    client: Arc<AgentClient>,
    agtuuid: String,
    command: String,
    timeout: u64,
) -> Result<()> {
    let ticket = client
        .send_ticket(ControlFormTicket {
            dst: agtuuid,
            form: ControlForm::SyncProcess(SyncProcess {
                command: CommandArg::Single(command),
                timeout: timeout as i64,
                stdout: None, stderr: None, status: None,
                start_time: None, elapsed_time: None,
                error: None, objuuid: None, coluuid: None,
            }),
            ..ControlFormTicket::default()
        })
        .await?;

    let ticket = poll_ticket(Arc::clone(&client), ticket, timeout * 2).await;

    if let ControlForm::SyncProcess(ref f) = ticket.form {
        if let Some(ref out) = f.stdout { print!("{}", out.trim_end_matches('\n')); }
        if let Some(ref err) = f.stderr { eprint!("{}", err.trim_end_matches('\n')); }
        if let Some(status) = f.status {
            if status != 0 { process::exit(status as i32); }
        }
    }

    if let Some(ref e) = ticket.error {
        eprintln!("{e}");
        process::exit(1);
    }

    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli    = Cli::parse();
    let config = Config::load();
    let client = Arc::new(AgentClient::new(config.client_control_url.clone()));

    match cli.command {
        Commands::Discover { peer_url, polling, delay, ttl } =>
            cmd_discover(client, peer_url, polling, delay, ttl).await?,

        Commands::Delete { delete_all, agtuuid } =>
            cmd_delete(client, delete_all, agtuuid).await?,

        Commands::Stat { agtuuid, timeout } =>
            cmd_stat(client, agtuuid, timeout).await?,

        Commands::Bench { agtuuid, timeout } =>
            cmd_bench(client, agtuuid, timeout).await?,

        Commands::Put { src_path, dst_path, timeout, src_agtuuid, dst_agtuuid } =>
            cmd_put(client, src_path, dst_path, timeout, src_agtuuid, dst_agtuuid).await?,

        Commands::Run { agtuuid, command, timeout } =>
            cmd_run(client, agtuuid, command, timeout).await?,
    }

    Ok(())
}
