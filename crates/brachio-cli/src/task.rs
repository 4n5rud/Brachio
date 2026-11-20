//! `brachio task create`: how an agent asks for a child task.

use anyhow::{bail, Result};
use brachio_runtime::mailbox::{MailRequest, MailResponse, CREATE_TASK_TIMEOUT};
use clap::{Args, Subcommand};

use crate::mail;

#[derive(Subcommand)]
pub enum TaskCommand {
    /// Ask the user to create a child task with its own worktree and branch. The user is asked
    /// first; nothing is created unless they allow it. Prints the new task's terminal key.
    Create(CreateArgs),
}

#[derive(Args)]
pub struct CreateArgs {
    /// Title of the new task. The branch name is derived from it.
    #[arg(long)]
    title: String,
    /// What the new task's agent should do. `-` reads it from standard input.
    #[arg(long)]
    prompt: String,
    /// claude or codex.
    #[arg(long)]
    agent: Option<String>,
    /// Commit-ish to branch from. Defaults to this task's branch.
    #[arg(long)]
    base: Option<String>,
}

pub fn run(command: TaskCommand) -> Result<()> {
    match command {
        TaskCommand::Create(args) => create(args),
    }
}

fn create(args: CreateArgs) -> Result<()> {
    let (endpoint, key) = mail::endpoint_and_key()?;
    let prompt = if args.prompt == "-" {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
        text.replace("\r\n", "\n")
    } else {
        args.prompt
    };
    let request = MailRequest::CreateTask {
        key,
        title: args.title,
        prompt,
        agent: args.agent,
        base: args.base,
    };
    let MailResponse::TaskCreated { key, branch, .. } =
        mail::call_within(&endpoint, &request, CREATE_TASK_TIMEOUT)?
    else {
        bail!("Brachio answered with something unexpected.");
    };
    println!("created {key} ({branch})");
    Ok(())
}
