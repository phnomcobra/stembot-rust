//! CLI control interface for agent management and network operations.
//!
//! Mirrors Python's `stembot/control.py`.

use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};

use stembot_rust::{
    cli,
    executor::agent::AgentClient,
    models::config::Config,
};

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[clap(name = "agt-control", about = "Agent control and network management")]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Discover and establish connection with a peer agent
    Discover {
        /// URL of the peer agent to discover (e.g., http://peer:8080/mpi)
        peer_url: String,
        /// Enable polling mode for continuous discovery updates
        #[clap(short = 'p', long)]
        polling: bool,
        /// Delay making discovery request for n seconds
        #[clap(short = 'd', long)]
        delay: Option<u64>,
        /// Time-to-live for the discovery in seconds
        #[clap(long)]
        ttl: Option<f64>,
    },
    /// Remove agents from the network
    Delete {
        /// Delete all agents
        #[clap(long = "all")]
        delete_all: bool,
        /// Delete a specific agent by UUID
        #[clap(long)]
        agtuuid: Option<String>,
    },
    /// Retrieve and display agent statistics
    Stat {
        /// UUID of the agent to query
        agtuuid: String,
        /// Timeout in seconds (default: 15)
        #[clap(short = 't', long, default_value = "15")]
        timeout: u64,
    },
    /// Benchmark agent file I/O performance across multiple file sizes
    Bench {
        /// UUID of the agent to benchmark
        agtuuid: String,
        /// Timeout in seconds per operation (default: 15)
        #[clap(short = 't', long, default_value = "15")]
        timeout: u64,
    },
    /// Transfer a file from source to destination
    Put {
        /// Source file path
        src_path: String,
        /// Destination file path
        dst_path: String,
        /// Timeout in seconds (default: 15)
        #[clap(short = 't', long, default_value = "15")]
        timeout: u64,
        /// Source agent UUID (reads from local filesystem if omitted)
        #[clap(short = 's', long)]
        src_agtuuid: Option<String>,
        /// Destination agent UUID (writes to local filesystem if omitted)
        #[clap(short = 'd', long)]
        dst_agtuuid: Option<String>,
    },
    /// Execute a command on a remote agent
    Run {
        /// UUID of the agent to run the command on
        agtuuid: String,
        /// Command to execute
        command: String,
        /// Timeout in seconds (default: 15)
        #[clap(short = 't', long, default_value = "15")]
        timeout: u64,
    },
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli    = Cli::parse();
    let config = Config::load();
    let client = Arc::new(AgentClient::new(config.client_control_url.clone()));

    match cli.command {
        Commands::Discover { peer_url, polling, delay, ttl } =>
            cli::discover::cmd_discover(client, peer_url, polling, delay, ttl).await?,

        Commands::Delete { delete_all, agtuuid } =>
            cli::delete::cmd_delete(client, delete_all, agtuuid).await?,

        Commands::Stat { agtuuid, timeout } =>
            cli::stat::cmd_stat(client, agtuuid, timeout).await?,

        Commands::Bench { agtuuid, timeout } =>
            cli::bench::cmd_bench(client, agtuuid, timeout).await?,

        Commands::Put { src_path, dst_path, timeout, src_agtuuid, dst_agtuuid } =>
            cli::put::cmd_put(client, src_path, dst_path, timeout, src_agtuuid, dst_agtuuid).await?,

        Commands::Run { agtuuid, command, timeout } =>
            cli::run::cmd_run(client, agtuuid, command, timeout).await?,
    }

    Ok(())
}
