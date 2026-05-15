pub mod bench;
pub mod delete;
pub mod discover;
pub mod put;
pub mod run;
pub mod stat;

use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::{
    executor::agent::AgentClient,
    models::control::{CheckTicket, CloseTicket, ControlForm, ControlFormTicket},
};

pub const KB: usize = 1_024;
pub const MB: usize = 1_024 * 1_024;
pub const GB: usize = 1_024 * 1_024 * 1_024;

pub fn format_bytes(n: f64) -> String {
    if n < KB as f64       { format!("{n:.0} B") }
    else if n < MB as f64  { format!("{:.1} KB", n / KB as f64) }
    else if n < GB as f64  { format!("{:.1} MB", n / MB as f64) }
    else                   { format!("{:.1} GB", n / GB as f64) }
}

pub fn format_bandwidth(bps: f64) -> String {
    if bps < KB as f64      { format!("{bps:.0} B/s") }
    else if bps < MB as f64 { format!("{:.1} KB/s", bps / KB as f64) }
    else if bps < GB as f64 { format!("{:.1} MB/s", bps / MB as f64) }
    else                    { format!("{:.1} GB/s", bps / GB as f64) }
}

/// Poll a ticket until serviced or timeout, then read and close it.
///
/// 1. Uses `CheckTicket` (lightweight) to poll for service completion.
/// 2. Once serviced, reads the full ticket via `read_ticket`.
/// 3. Closes the ticket via `CloseTicket`.
pub async fn poll_ticket(
    client: Arc<AgentClient>,
    ticket: ControlFormTicket,
    timeout_secs: u64,
) -> ControlFormTicket {
    let start = std::time::Instant::now();

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
