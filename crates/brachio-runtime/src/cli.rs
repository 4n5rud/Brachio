//! The `serve` and `send` command line of the runtime executable.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use brachio_runtime::endpoint::{self, Liveness};
use brachio_runtime::protocol;
use brachio_runtime::transport;
use brachio_runtime::wire::{
    await_outcome, read_message, write_message, Action, Request, IO_TIMEOUT, MAX_MESSAGE,
    MAX_RESPONSE,
};
use clap::{Parser, Subcommand};
use interprocess::local_socket::tokio::Stream;
use tokio::io::BufReader;

use crate::server;

#[derive(Parser)]
pub(crate) struct Args {
    /// Local pipe name for this runtime instance.
    #[arg(long)]
    pub(crate) pipe: String,
    /// Where the last screen of each shell is kept. Defaults to the user's state directory.
    #[arg(long)]
    pub(crate) state_dir: Option<PathBuf>,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Own PTYs until this process exits.
    Serve,
    /// Send one request to a running instance.
    Send {
        /// Protocol version to speak, for testing older clients.
        #[arg(long, default_value_t = protocol::CURRENT)]
        protocol: u8,
        #[command(subcommand)]
        action: Action,
    },
}

#[tokio::main]
pub(crate) async fn run() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Serve => server::serve(&args.pipe, args.state_dir).await,
        Command::Send { protocol, action } => {
            let connected = transport::connect(&args.pipe).await;
            let reason = match endpoint::classify(&connected) {
                Liveness::Dead => "terminal runtime is not running",
                _ => "cannot tell whether the terminal runtime is running",
            };
            request(connected.context(reason)?, protocol, action).await
        }
    }
}

pub(crate) async fn request(mut stream: Stream, protocol: u8, action: Action) -> Result<()> {
    let changes_state = action.changes_state();
    let wait_for_completion = action.is_session_lifecycle();
    let following = matches!(action, Action::Follow { .. } | Action::FollowFrame { .. });
    tokio::time::timeout(
        IO_TIMEOUT,
        write_message(
            &mut stream,
            &Request {
                version: protocol,
                action,
            },
            MAX_MESSAGE,
        ),
    )
    .await??;
    let mut reader = BufReader::new(stream);
    let reason = if changes_state {
        "runtime disconnected before confirming the operation outcome"
    } else {
        "runtime closed before responding"
    };
    let first = await_outcome(wait_for_completion, read_message(&mut reader, MAX_RESPONSE))
        .await
        .context(reason)?
        .context(reason)?
        .context(reason)?;
    let completed = print_response(&first, protocol)?;
    if following {
        if completed {
            return Ok(());
        }
        while let Some(message) = read_message(&mut reader, MAX_RESPONSE).await? {
            if print_response(&message, protocol)? {
                return Ok(());
            }
        }
        bail!("runtime disconnected before the terminal exited");
    }
    Ok(())
}

pub(crate) fn print_response(message: &[u8], protocol: u8) -> Result<bool> {
    use std::io::Write;

    let response: serde_json::Value = serde_json::from_slice(message)?;
    let mut output = std::io::stdout().lock();
    output.write_all(message)?;
    output.flush()?;
    if let Some(error) = response.get("error") {
        bail!("{error}");
    }
    if response.get("version").and_then(serde_json::Value::as_u64) != Some(u64::from(protocol)) {
        bail!("incompatible runtime response");
    }
    let output = &response["output"];
    let completed = response["session"]["exited"] == true
        && output["next"]
            .as_u64()
            .is_some_and(|next| Some(next) == output["end"].as_u64());
    Ok(completed)
}
