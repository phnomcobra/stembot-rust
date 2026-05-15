use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;

use crate::{
    executor::{
        agent::AgentClient,
        file::{load_file_to_form, write_file_from_form},
    },
    models::control::{ControlForm, ControlFormTicket, LoadFile, WriteFile},
};

use super::poll_ticket;

pub async fn cmd_put(
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
