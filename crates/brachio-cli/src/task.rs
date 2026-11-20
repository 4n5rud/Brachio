//! `brachio task create`: how an agent asks for a child task.

use anyhow::{bail, Result};
use std::time::Duration;

use brachio_runtime::mailbox::{
    ChildTaskInfo, MailRequest, MailResponse, ReportedChild, CALL_TIMEOUT, CREATE_TASK_TIMEOUT,
    DEFAULT_WAIT_SECS, MAX_WAIT_SECS,
};
use clap::{ArgGroup, Args, Subcommand};

use crate::mail;

/// Exit status of `task wait` when time ran out, apart from 1 for errors.
const TIMEOUT_EXIT: i32 = 3;
/// How much longer than the wait the command waits for the app's answer.
const ANSWER_SLACK: Duration = Duration::from_secs(10);

#[derive(Subcommand)]
pub enum TaskCommand {
    /// List the child tasks of this task: key, title, state and unread messages.
    List,
    /// Wait, inside the app and without polling, until the open child tasks have reported
    /// `worker_done`. Prints the key and subject of each child that reported. Exits with status 3
    /// on timeout.
    Wait(WaitArgs),
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

#[derive(Args)]
#[command(group(ArgGroup::new("mode").args(["any", "all"])))]
pub struct WaitArgs {
    /// Return as soon as one child has reported.
    #[arg(long)]
    any: bool,
    /// Wait for every open child. This is the default.
    #[arg(long)]
    all: bool,
    /// Longest wait in seconds, at most 3600.
    #[arg(long, default_value_t = DEFAULT_WAIT_SECS, value_parser = clap::value_parser!(u64).range(1..=MAX_WAIT_SECS))]
    timeout_secs: u64,
}

pub fn run(command: TaskCommand) -> Result<()> {
    match command {
        TaskCommand::Create(args) => create(args),
        TaskCommand::List => list(),
        TaskCommand::Wait(args) => wait(&args),
    }
}

pub(crate) fn children() -> Result<Vec<ChildTaskInfo>> {
    let (endpoint, key) = mail::endpoint_and_key()?;
    let MailResponse::TaskList { children } =
        mail::call_within(&endpoint, &MailRequest::ListTasks { key }, CALL_TIMEOUT)?
    else {
        bail!("Brachio answered with something unexpected.");
    };
    Ok(children)
}

fn list() -> Result<()> {
    print!("{}", format_children(&children()?));
    Ok(())
}

fn format_children(children: &[ChildTaskInfo]) -> String {
    if children.is_empty() {
        return "no child tasks\n".to_owned();
    }
    children
        .iter()
        .map(|child| {
            let state = match (child.archived, child.reported) {
                (true, true) => "archived reported",
                (true, false) => "archived",
                (false, true) => "open reported",
                (false, false) => "open",
            };
            format!(
                "{}  {}  {state}  unread={}\n",
                child.key, child.title, child.unread
            )
        })
        .collect()
}

fn wait(args: &WaitArgs) -> Result<()> {
    let (endpoint, key) = mail::endpoint_and_key()?;
    let request = MailRequest::WaitTasks {
        key,
        all: !args.any,
        timeout_secs: args.timeout_secs,
    };
    let limit = Duration::from_secs(args.timeout_secs) + ANSWER_SLACK;
    let MailResponse::TasksReported {
        reported,
        pending,
        timed_out,
    } = mail::call_within(&endpoint, &request, limit)?
    else {
        bail!("Brachio answered with something unexpected.");
    };
    print!("{}", format_wait(&reported, &pending, timed_out));
    if timed_out {
        std::process::exit(TIMEOUT_EXIT);
    }
    Ok(())
}

fn format_wait(reported: &[ReportedChild], pending: &[String], timed_out: bool) -> String {
    let mut text = String::new();
    if timed_out {
        text.push_str("timeout\n");
    } else if reported.is_empty() {
        return "no child tasks\n".to_owned();
    }
    for child in reported {
        text.push_str(&format!("{} {}\n", child.key, child.subject));
    }
    if timed_out {
        text.push_str(&format!("pending: {}\n", pending.join(" ")));
    }
    text
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
