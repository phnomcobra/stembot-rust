use std::sync::Arc;

use anyhow::Result;
use tokio::time::{sleep, Duration};

use crate::{
    executor::agent::AgentClient,
    models::control::{Benchmark, CheckTicket, CloseTicket, ControlForm, ControlFormTicket},
};

use super::{format_bandwidth, format_bytes, KB, MB};

/// Poll a `Benchmark` ticket for timing data only (no content read).
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

pub async fn cmd_bench(
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
