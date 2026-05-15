use std::sync::Arc;

use anyhow::Result;
use tokio::time::{sleep, Duration};

use crate::{
    executor::agent::AgentClient,
    models::control::{ControlForm, DiscoverPeer},
};

pub async fn cmd_discover(
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
