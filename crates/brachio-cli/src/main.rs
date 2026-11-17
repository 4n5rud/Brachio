//! `brachio` helper binary. Agents' hooks run `brachio notify`; agents start
//! `brachio mcp` as an MCP server. Both talk to the running app over a local
//! socket identified by `BRACHIO_SOCKET` and authenticated by `BRACHIO_TOKEN`.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "brachio", version, about = "Brachio helper commands")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Forward a hook event (JSON on stdin) to the running app.
    Notify {
        /// Hook event name, for example `Stop` or `PreToolUse`.
        #[arg(long)]
        event: String,
    },
    /// Run the Brachio MCP server on stdio.
    Mcp,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if std::env::var_os("BRACHIO_SOCKET").is_none() {
        bail!("BRACHIO_SOCKET is not set; run this from a session started by Brachio");
    }
    match cli.command {
        Command::Notify { event } => bail!("notify is not implemented yet (event {event}); M1"),
        Command::Mcp => bail!("mcp is not implemented yet; M3"),
    }
}
