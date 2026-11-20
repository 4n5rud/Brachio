//! `brachio` helper binary that agents run. `brachio hook permission-request` forwards an
//! agent's permission question to the app; `brachio mcp` serves the task tools.

mod mail;
mod mail_hook;
mod mcp_tools;
mod task;

use std::io::Read;
use std::time::Duration;

use anyhow::{bail, Result};
use brachio_runtime::approval::{
    self, ApprovalRequest, DEFAULT_WAIT, ENDPOINT_VARIABLE, KEY_VARIABLE, WAIT_VARIABLE,
};
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
    /// Send a message to another task's agent.
    Send(mail::SendArgs),
    /// Send a question and return immediately.
    Ask(mail::AskArgs),
    /// Answer an existing message in its thread.
    Reply(mail::ReplyArgs),
    /// Read the messages other agents sent to this one.
    Check(mail::CheckArgs),
    /// List recent messages to and from this agent, read or not.
    Inbox(mail::InboxArgs),
    /// List the open tasks a message can be sent to.
    Peers,
    /// Ask the user to start other tasks.
    Task {
        #[command(subcommand)]
        command: task::TaskCommand,
    },
    /// Commands that an agent's hooks run.
    Hook {
        #[command(subcommand)]
        hook: Hook,
    },
}

#[derive(Subcommand)]
enum Hook {
    /// Let the user answer a permission question in the app. Prints the agent's answer, or
    /// nothing when the app does not answer, so the agent asks in the terminal as usual.
    PermissionRequest,
    /// Tell the agent, once per new message, that mail is waiting. Prints nothing otherwise.
    Mail,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Hook { hook: Hook::Mail } => {
            // Like the permission hook, it must never block the agent.
            mail_hook::run();
            Ok(())
        }
        Command::Hook {
            hook: Hook::PermissionRequest,
        } => {
            // Exit status 2 would block the agent's tool. Whatever happens, finish normally.
            permission_request();
            Ok(())
        }
        Command::Send(args) => mail::send(args),
        Command::Ask(args) => mail::ask(args),
        Command::Reply(args) => mail::reply(args),
        Command::Check(args) => mail::check(&args),
        Command::Inbox(args) => mail::inbox(&args),
        Command::Peers => mail::peers(),
        Command::Task { command } => task::run(command),
        Command::Notify { event } => bail!("notify is not implemented yet (event {event}); M1"),
        Command::Mcp => {
            brachio_mcp::serve(
                std::io::stdin().lock(),
                std::io::stdout().lock(),
                mcp_tools::McpTools,
            )?;
            Ok(())
        }
    }
}

fn permission_request() {
    let (Some(endpoint), Some(key)) = (
        std::env::var_os(ENDPOINT_VARIABLE),
        std::env::var_os(KEY_VARIABLE),
    ) else {
        return;
    };
    let (Some(endpoint), Some(key)) = (endpoint.to_str(), key.to_str()) else {
        return;
    };
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return;
    }
    let Some(request) = approval::request_from_hook_input(&input, key) else {
        return;
    };
    if let Some(output) =
        answer(endpoint, &request).and_then(|decision| approval::hook_output(&decision))
    {
        println!("{output}");
    }
}

fn answer(endpoint: &str, request: &ApprovalRequest) -> Option<approval::Decision> {
    let wait = std::env::var(WAIT_VARIABLE)
        .ok()
        .and_then(|seconds| seconds.parse::<u64>().ok())
        .map_or(DEFAULT_WAIT, Duration::from_secs);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    runtime.block_on(approval::ask(endpoint, request, wait))
}
