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
    /// Hand this child task's committed work over for the user's review. The app records the
    /// current commit and changed files itself; the task is not complete until the user approves.
    Handoff(HandoffArgs),
}

#[derive(Args)]
pub struct HandoffArgs {
    /// What was done, in a few sentences.
    #[arg(long)]
    summary: String,
    /// JSON array of strings naming the tests you ran, such as '["cargo test"]'. The user also
    /// sees whether Brachio itself verified a QA run.
    #[arg(long)]
    tests: Option<String>,
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
    /// Key of a sibling child task that must be completed and approved first. Repeat for
    /// several. Needs "Start automatically" turned on for this task in the Tasks tab.
    #[arg(long)]
    after: Vec<String>,
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
        TaskCommand::Handoff(args) => handoff(&args),
        TaskCommand::List => list(),
        TaskCommand::Wait(args) => wait(&args),
    }
}

fn list() -> Result<()> {
    let (endpoint, key) = mail::endpoint_and_key()?;
    let MailResponse::TaskList { children } =
        mail::call_within(&endpoint, &MailRequest::ListTasks { key }, CALL_TIMEOUT)?
    else {
        bail!("Brachio answered with something unexpected.");
    };
    print!("{}", format_children(&children));
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
        after: args.after,
    };
    let MailResponse::TaskCreated { key, branch, .. } =
        mail::call_within(&endpoint, &request, CREATE_TASK_TIMEOUT)?
    else {
        bail!("Brachio answered with something unexpected.");
    };
    println!("created {key} ({branch})");
    Ok(())
}

fn parse_tests(text: Option<&str>) -> Result<Vec<String>> {
    match text {
        None => Ok(Vec::new()),
        Some(text) => serde_json::from_str(text)
            .map_err(|_| anyhow::anyhow!("--tests must be a JSON array of strings.")),
    }
}

fn handoff(args: &HandoffArgs) -> Result<()> {
    let tests = parse_tests(args.tests.as_deref())?;
    let (endpoint, key) = mail::endpoint_and_key()?;
    let request = MailRequest::Handoff {
        key,
        summary: args.summary.clone(),
        tests,
    };
    let MailResponse::HandedOff {
        commit_sha,
        changed_paths,
    } = mail::call_within(&endpoint, &request, CALL_TIMEOUT)?
    else {
        bail!("Brachio answered with something unexpected.");
    };
    println!(
        "handed off {} ({changed_paths} changed files); waiting for the user's review",
        &commit_sha[..commit_sha.len().min(12)]
    );
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::parse_tests;

    #[test]
    fn tests_must_be_a_json_array_of_strings() {
        assert_eq!(parse_tests(None).unwrap(), Vec::<String>::new());
        assert_eq!(
            parse_tests(Some(r#"["cargo test"]"#)).unwrap(),
            vec!["cargo test".to_owned()]
        );
        assert!(parse_tests(Some("cargo test")).is_err());
        assert!(parse_tests(Some(r#"{"a":1}"#)).is_err());
    }
}
