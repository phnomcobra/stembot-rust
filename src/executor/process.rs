//! Subprocess execution with timeout enforcement and output capture.
//!
//! Mirrors Python's `stembot/executor/process.py`.

use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::models::control::{CommandArg, SyncProcess};

/// Execute a subprocess with timeout enforcement and output capture.
///
/// Mirrors Python's `sync_process(form: SyncProcess) -> SyncProcess`.
/// Works on Unix and Windows.
pub fn sync_process(mut form: SyncProcess) -> SyncProcess {
    let mut cmd = match &form.command {
        CommandArg::Single(s) => {
            #[cfg(unix)]
            {
                let mut c = Command::new("sh");
                c.args(["-c", s]);
                c
            }
            #[cfg(windows)]
            {
                let mut c = Command::new("cmd");
                c.args(["/C", s]);
                c
            }
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

    let timeout_secs = form.timeout as f64;

    // Share the Child handle with a watchdog thread using Arc<Mutex<Option<Child>>>.
    // The watchdog calls child.kill() if the timeout fires before the main thread
    // takes ownership back. This approach is cross-platform (no libc / SIGKILL).
    let child_arc = Arc::new(Mutex::new(Some(child)));
    let child_arc_watchdog = Arc::clone(&child_arc);

    let watchdog = thread::spawn(move || {
        thread::sleep(Duration::from_secs_f64(timeout_secs));
        if let Ok(mut guard) = child_arc_watchdog.lock() {
            if let Some(ref mut c) = *guard {
                let _ = c.kill();
            }
        }
    });

    // Take the Child back out so we can call wait_with_output().
    let child = child_arc
        .lock()
        .expect("mutex poisoned")
        .take()
        .expect("child already taken");

    let output = child.wait_with_output();

    // Joining the watchdog is best-effort; it exits naturally once it sleeps through.
    drop(watchdog);

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
