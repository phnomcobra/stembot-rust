use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::{
    executor::{agent::AgentClient, file::load_form_from_bytes},
    models::control::{
        CommandArg, ControlForm, ControlFormTicket, CreateUserKey, GetConfig, GetKeyMetadata,
        GetPeers, GetRoutes, LoadFile, SyncProcess,
    },
};

use super::poll_ticket;

pub async fn cmd_test(client: Arc<AgentClient>, destinations: String, timeout: u64) -> Result<()> {
    let destinations = destinations
        .split(',')
        .map(str::trim)
        .filter(|destination| !destination.is_empty())
        .collect::<Vec<_>>();
    if destinations.is_empty() {
        return Err(anyhow!("no destinations provided"));
    }

    let mut passed = 0;
    let mut failed = 0;
    for destination in destinations {
        let mut write_form = load_form_from_bytes(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890");
        write_form.path = "test.txt".to_string();
        let forms = vec![
            (ControlForm::WriteFile(write_form), false),
            (ControlForm::GetPeers(GetPeers::default()), false),
            (ControlForm::GetRoutes(GetRoutes::default()), false),
            (ControlForm::GetConfig(GetConfig::default()), false),
            (
                ControlForm::GetKeyMetadata(GetKeyMetadata::default()),
                false,
            ),
            (
                ControlForm::CreateUserKey(CreateUserKey {
                    agtuuid: Some("test".to_string()),
                    objuuid: "test".to_string(),
                    passphrase: "test".to_string(),
                    ..Default::default()
                }),
                false,
            ),
            (
                ControlForm::SyncProcess(SyncProcess {
                    command: CommandArg::Single("ls /".to_string()),
                    ..Default::default()
                }),
                false,
            ),
            (
                ControlForm::SyncProcess(SyncProcess {
                    command: CommandArg::Multi(vec!["bad".to_string(), "command".to_string()]),
                    ..Default::default()
                }),
                true,
            ),
            (
                ControlForm::LoadFile(LoadFile {
                    path: "test.txt".to_string(),
                    ..Default::default()
                }),
                false,
            ),
            (
                ControlForm::LoadFile(LoadFile {
                    path: "test_bad.txt".to_string(),
                    ..Default::default()
                }),
                true,
            ),
        ];

        for (index, (form, expect_failure)) in forms.into_iter().enumerate() {
            let form_name = form.form_type();
            let sent = match client
                .send_ticket(ControlFormTicket {
                    dst: destination.to_string(),
                    tracing: true,
                    form,
                    ..Default::default()
                })
                .await
            {
                Ok(ticket) => ticket,
                Err(error) => {
                    eprintln!("FAIL {destination} test {index} ({form_name}): {error}");
                    failed += 1;
                    continue;
                }
            };
            let result = poll_ticket(Arc::clone(&client), sent, timeout).await;
            let form_error = match &result.form {
                ControlForm::WriteFile(form) => form.error.as_deref(),
                ControlForm::GetPeers(form) => form.error.as_deref(),
                ControlForm::GetRoutes(form) => form.error.as_deref(),
                ControlForm::GetConfig(form) => form.error.as_deref(),
                ControlForm::GetKeyMetadata(form) => form.error.as_deref(),
                ControlForm::CreateUserKey(form) => form.error.as_deref(),
                ControlForm::SyncProcess(form) => form.error.as_deref(),
                ControlForm::LoadFile(form) => form.error.as_deref(),
                _ => None,
            };
            let success = result.service_time.is_some()
                && (expect_failure || (result.error.is_none() && form_error.is_none()));
            if success {
                println!("ok {destination} test {index} ({form_name})");
                passed += 1;
            } else {
                eprintln!(
                    "FAIL {destination} test {index} ({form_name}): ticket_error={:?}, form_error={form_error:?}",
                    result.error
                );
                failed += 1;
            }
        }
    }

    println!("\n{passed} passed; {failed} failed");
    if failed > 0 {
        return Err(anyhow!("{failed} integration test(s) failed"));
    }
    Ok(())
}
