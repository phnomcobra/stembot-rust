use std::sync::Arc;

use anyhow::Result;

use crate::{
    executor::agent::AgentClient,
    models::control::{ControlForm, DeletePeers},
};

pub async fn cmd_delete(
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
