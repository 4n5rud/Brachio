//! `brachio send`, `check`, `inbox` and `peers`: how an agent talks to other agents' tasks.
//!
//! Output is short on purpose, because an agent reads it as context: one line per message and a
//! 200 character preview of the body unless `--full` is given.

use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use brachio_runtime::approval::{ENDPOINT_VARIABLE, KEY_VARIABLE};
use brachio_runtime::mailbox::{self, MailMessage, MailRequest, MailResponse, SendMail};
use clap::Args;

const PREVIEW_CHARS: usize = 200;
const WAIT_POLL: Duration = Duration::from_secs(1);

#[derive(Args)]
pub struct SendArgs {
    /// Who gets it: a key from `brachio peers`, a task title, or the start of a task id.
    #[arg(long)]
    to: String,
    /// One line saying what this is about.
    #[arg(long)]
    subject: String,
    /// Details. `-` reads them from standard input. Keep it short; put long text in a file and
    /// send its path.
    #[arg(long, default_value = "")]
    body: String,
    /// status, dispatch, worker_done, merge_ready, escalation, handoff, decision_gate, question
    /// or heartbeat.
    #[arg(long = "type", default_value = "status")]
    kind: String,
    /// low, normal or high.
    #[arg(long)]
    priority: Option<String>,
    /// The message id this answers.
    #[arg(long)]
    reply_to: Option<String>,
    /// Sending the same id again does not send a second message.
    #[arg(long)]
    request_id: Option<String>,
    /// File paths or other references, as JSON text.
    #[arg(long)]
    payload: Option<String>,
}

#[derive(Args)]
pub struct CheckArgs {
    /// Most messages to take at once.
    #[arg(long)]
    limit: Option<usize>,
    /// Look without marking the messages as read.
    #[arg(long)]
    peek: bool,
    /// Print whole bodies instead of a preview.
    #[arg(long)]
    full: bool,
    /// Wait until a message arrives instead of returning at once.
    #[arg(long)]
    wait: bool,
    /// With `--wait`, give up after this many seconds.
    #[arg(long, default_value_t = 600)]
    timeout_secs: u64,
}

#[derive(Args)]
pub struct InboxArgs {
    /// Most messages to list.
    #[arg(long)]
    limit: Option<usize>,
    /// Print whole bodies instead of a preview.
    #[arg(long)]
    full: bool,
}

fn endpoint_and_key() -> Result<(String, String)> {
    let endpoint = std::env::var(ENDPOINT_VARIABLE);
    let key = std::env::var(KEY_VARIABLE);
    match (endpoint, key) {
        (Ok(endpoint), Ok(key)) => Ok((endpoint, key)),
        _ => bail!("Not running inside a Brachio agent terminal, so there is no one to send as."),
    }
}

fn call(endpoint: &str, request: &MailRequest) -> Result<MailResponse> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    match runtime.block_on(mailbox::call(endpoint, request))? {
        MailResponse::Error { message } => Err(anyhow!(message)),
        other => Ok(other),
    }
}

pub fn send(args: SendArgs) -> Result<()> {
    let (endpoint, from) = endpoint_and_key()?;
    let body = if args.body == "-" {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
        text
    } else {
        args.body
    };
    let request = MailRequest::Send(SendMail {
        from,
        to: args.to,
        kind: args.kind,
        priority: args.priority,
        subject: args.subject,
        body,
        payload: args.payload,
        reply_to: args.reply_to,
        request_id: args.request_id,
    });
    let MailResponse::Sent {
        id, to, duplicate, ..
    } = call(&endpoint, &request)?
    else {
        bail!("Brachio answered with something unexpected.");
    };
    let note = if duplicate { " (already sent)" } else { "" };
    println!("sent {id} to {to}{note}");
    Ok(())
}

pub fn check(args: &CheckArgs) -> Result<()> {
    let (endpoint, key) = endpoint_and_key()?;
    if args.wait {
        wait_for_mail(&endpoint, &key, Duration::from_secs(args.timeout_secs))?;
    }
    let request = MailRequest::Check {
        key,
        limit: args.limit,
        peek: args.peek,
    };
    let MailResponse::Messages {
        messages,
        unread_left,
    } = call(&endpoint, &request)?
    else {
        bail!("Brachio answered with something unexpected.");
    };
    print_messages(&messages, unread_left, args.full);
    Ok(())
}

/// Returns once something is waiting or the time is up, without taking anything.
fn wait_for_mail(endpoint: &str, key: &str, limit: Duration) -> Result<()> {
    let deadline = Instant::now() + limit;
    loop {
        let look = MailRequest::Check {
            key: key.to_owned(),
            limit: Some(1),
            peek: true,
        };
        let MailResponse::Messages { messages, .. } = call(endpoint, &look)? else {
            bail!("Brachio answered with something unexpected.");
        };
        if !messages.is_empty() || Instant::now() >= deadline {
            return Ok(());
        }
        std::thread::sleep(WAIT_POLL);
    }
}

pub fn inbox(args: &InboxArgs) -> Result<()> {
    let (endpoint, key) = endpoint_and_key()?;
    let request = MailRequest::Inbox {
        key,
        limit: args.limit,
    };
    let MailResponse::Messages {
        messages,
        unread_left,
    } = call(&endpoint, &request)?
    else {
        bail!("Brachio answered with something unexpected.");
    };
    print_messages(&messages, unread_left, args.full);
    Ok(())
}

pub fn peers() -> Result<()> {
    let (endpoint, key) = endpoint_and_key()?;
    let MailResponse::Peers { peers } = call(&endpoint, &MailRequest::Peers { key })? else {
        bail!("Brachio answered with something unexpected.");
    };
    if peers.is_empty() {
        println!("no other open tasks");
    }
    for peer in peers {
        println!("{}  {}  ({})", peer.key, peer.title, peer.branch);
    }
    Ok(())
}

fn print_messages(messages: &[MailMessage], unread_left: u32, full: bool) {
    if messages.is_empty() {
        println!("no new messages");
        return;
    }
    for message in messages {
        println!("{}", format_message(message, full));
    }
    if unread_left > 0 {
        println!("{unread_left} more unread; run brachio check again");
    }
}

fn format_message(message: &MailMessage, full: bool) -> String {
    let from = message.from_title.as_deref().unwrap_or(&message.from);
    let mut text = format!(
        "{} [{}{}] from {from}: {}",
        message.id,
        message.kind,
        if message.priority == "normal" {
            String::new()
        } else {
            format!(", {}", message.priority)
        },
        message.subject
    );
    if !message.body.is_empty() {
        let body: String = if full {
            message.body.clone()
        } else {
            let cut: String = message.body.chars().take(PREVIEW_CHARS).collect();
            if message.body.chars().count() > PREVIEW_CHARS {
                format!("{cut}… (--full for the rest)")
            } else {
                cut
            }
        };
        text.push('\n');
        text.push_str(&body);
    }
    if let Some(payload) = &message.payload {
        text.push_str(&format!("\npayload: {payload}"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(body: &str) -> MailMessage {
        MailMessage {
            id: "m1".into(),
            thread_id: "m1".into(),
            from: "t:agent".into(),
            from_title: Some("Build API".into()),
            kind: "question".into(),
            priority: "normal".into(),
            subject: "Which port?".into(),
            body: body.into(),
            payload: None,
            created_at_ms: 0,
            read: false,
        }
    }

    #[test]
    fn a_long_body_is_cut_to_a_preview_unless_the_whole_is_asked_for() {
        let body = "x".repeat(500);
        let short = format_message(&message(&body), false);
        assert!(short.contains("--full"));
        assert!(short.chars().count() < 330);
        let whole = format_message(&message(&body), true);
        assert!(whole.contains(&body));
        assert!(whole.starts_with("m1 [question] from Build API: Which port?"));
    }
}
