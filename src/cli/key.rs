use std::{sync::Arc, time::Duration};

use anyhow::{anyhow, Result};
use tokio::time::sleep;

use crate::{
    executor::agent::AgentClient,
    models::control::{ControlForm, ControlFormTicket, CreateUserKey},
};

use super::poll_ticket;

pub async fn cmd_set_user_key(
    client: Arc<AgentClient>,
    dst_agtuuid: String,
    objuuid: String,
    passphrase: String,
    agtuuid: Option<String>,
    delay: Option<u64>,
    timeout: u64,
) -> Result<()> {
    println!("Setting user key '{objuuid}' on agent: {dst_agtuuid}");
    if let Some(delay) = delay {
        println!("Waiting {delay} seconds before creating user key...");
        sleep(Duration::from_secs(delay)).await;
    }

    let ticket = client
        .send_ticket(ControlFormTicket {
            dst: dst_agtuuid.clone(),
            form: ControlForm::CreateUserKey(CreateUserKey {
                objuuid: objuuid.clone(),
                passphrase: Some(passphrase),
                agtuuid,
                ..Default::default()
            }),
            ..Default::default()
        })
        .await?;
    let ticket = poll_ticket(client, ticket, timeout).await;

    if ticket.service_time.is_none() {
        return Err(anyhow!("user key ticket was not serviced"));
    }
    if let Some(error) = ticket.error {
        return Err(anyhow!("{error}"));
    }
    if let ControlForm::CreateUserKey(form) = ticket.form {
        if let Some(error) = form.error {
            return Err(anyhow!("{error}"));
        }
    }
    println!("User key '{objuuid}' set on agent {dst_agtuuid}");
    Ok(())
}
