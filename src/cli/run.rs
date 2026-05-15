use std::process;
use std::sync::Arc;

use anyhow::Result;

use crate::{
    executor::agent::AgentClient,
    models::control::{CommandArg, ControlForm, ControlFormTicket, SyncProcess},
};

use super::poll_ticket;

pub async fn cmd_run(
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
