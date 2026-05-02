//! Subprocess execution with timeout enforcement and output capture.
//!
//! Mirrors Python's `stembot/executor/process.py`.

use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::models::control::{CommandArg, SyncProcess};

/// Execute a subprocess with timeout enforcement and output capture.
///
/// Mirrors Python's `sync_process(form: SyncProcess) -> SyncProcess`.
pub fn sync_process(mut form: SyncProcess) -> SyncProcess {
    let mut cmd = match &form.command {
        CommandArg::Single(s) => {
            let mut c = Command::new("sh");
            c.args(["-c", s]);
            c
        }
        CommandArg::Multi(args) => {
            if args.is_empty() {
                form.error = Some("empty command list".to_string());
                return form;
            }
            let mut c = Command::new(&args[0]);
            c.args(&args[1..]);
            c
        }
    };

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let start_instant = Instant::now();
    form.start_time = Some(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64(),
    );

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            form.elapsed_time = Some(0.0);
            form.error = Some(e.to_string());
            return form;
        }
    };

    let pid = child.id();
    let timeout_secs = form.timeout as f64;

    // Spawn a timeout watchdog: if the process doesn't finish within `timeout_secs`,
    // send SIGKILL.  The `done_tx` channel cancels the watchdog on early exit.
    let (done_tx, done_rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        let deadline = Duration::from_secs_f64(timeout_secs);
        if done_rx.recv_timeout(deadline).is_err() {
            // Timed out — kill the process.
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
        }
    });

    let output = child.wait_with_output();
    // Signal watchdog that process is done (cancels the timer).
    let _ = done_tx.send(());

    form.elapsed_time = Some(start_instant.elapsed().as_secs_f64());

    match output {
        Ok(o) => {
            form.stdout = Some(String::from_utf8_lossy(&o.stdout).into_owned());
            form.stderr = Some(String::from_utf8_lossy(&o.stderr).into_owned());
            form.status = o.status.code().map(|c| c as i64);
        }
        Err(e) => {
            form.error = Some(e.to_string());
        }
    }

    form
}
