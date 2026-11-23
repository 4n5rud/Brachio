//! An editable summary of a task for handing its context to another agent. Brachio only
//! builds the text from the task's own records; the user edits it and decides to send it. It
//! is never sent or typed on its own, and it is kept small.

use brachio_git::{changed_paths, Git};
use brachio_store::{
    latest_qa_result, recent_messages, send_message, NewMessage, RepoRecord, TaskRecord,
    MAX_BODY_BYTES,
};
use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::mailbox::agent_owner;
use crate::workspace::Database;

/// The most the summary may take, in bytes (about 1,750 tokens). Under the mailbox body limit.
pub(crate) const SUMMARY_MAX_BYTES: usize = 7000;
const GOAL_MAX_BYTES: usize = 1200;
const LINE_MAX_BYTES: usize = 100;
const MAX_FILES: usize = 25;
const MAX_STATUS_LINES: usize = 15;
const MAX_QUESTIONS: usize = 4;

/// What the summary is built from, already read from the task.
#[derive(Debug, Default)]
pub(crate) struct SummaryInput {
    pub title: String,
    pub branch: String,
    pub goal: String,
    pub changed: Vec<String>,
    pub status_lines: Vec<String>,
    /// `(outcome, command, verified by Brachio)` of the newest QA run.
    pub qa: Option<(String, String, bool)>,
    /// Subjects of questions nobody answered yet.
    pub questions: Vec<String>,
}

fn cut(text: &str, max_bytes: usize) -> (&str, bool) {
    if text.len() <= max_bytes {
        return (text, false);
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], true)
}

fn list(lines: &[String], max: usize, marker: &str) -> String {
    if lines.is_empty() {
        return "none".to_owned();
    }
    let mut out: Vec<String> = lines
        .iter()
        .take(max)
        .map(|l| format!("- {}", cut(l, LINE_MAX_BYTES).0))
        .collect();
    if lines.len() > max {
        out.push(format!("- ... and {} more {marker}", lines.len() - max));
    }
    out.join("\n")
}

/// The summary text, at most [`SUMMARY_MAX_BYTES`].
pub(crate) fn render(input: &SummaryInput) -> String {
    let (goal, goal_cut) = cut(input.goal.trim(), GOAL_MAX_BYTES);
    let qa = match &input.qa {
        Some((outcome, command, verified)) => format!(
            "{outcome}: {} ({})",
            cut(command, LINE_MAX_BYTES).0,
            if *verified {
                "run by Brachio"
            } else {
                "reported by the agent"
            }
        ),
        None => "no QA run".to_owned(),
    };
    let text = format!(
        "Context handed over from the task \"{title}\" (branch {branch}).\n\
         This is a summary for you to check, not an instruction or an approval.\n\n\
         ## Goal\n{goal}{ellipsis}\n\n\
         ## Changed files\n{files}\n\n\
         ## Git status\n{status}\n\n\
         ## Last QA\n{qa}\n\n\
         ## Open questions\n{questions}\n",
        title = input.title,
        branch = input.branch,
        ellipsis = if goal_cut { " ..." } else { "" },
        files = list(&input.changed, MAX_FILES, "files"),
        status = list(&input.status_lines, MAX_STATUS_LINES, "lines"),
        questions = list(&input.questions, MAX_QUESTIONS, "questions"),
    );
    let (text, was_cut) = cut(&text, SUMMARY_MAX_BYTES);
    if was_cut {
        format!("{text}\n...")
    } else {
        text.to_owned()
    }
}

/// Questions sent to the task's agent that no later message answers.
fn open_questions(connection: &Connection, task_id: &str) -> Vec<String> {
    let key = format!("{task_id}:agent");
    let Ok(messages) = recent_messages(connection, &key, 100) else {
        return Vec::new();
    };
    messages
        .iter()
        .filter(|m| m.kind == "question" && m.to_key == key)
        .filter(|m| {
            !messages
                .iter()
                .any(|reply| reply.reply_to.as_deref() == Some(m.id.as_str()))
        })
        .map(|m| m.subject.clone())
        .collect()
}

/// Changed paths against the default branch and `git status` lines; empty for a plain folder.
fn read_git(task: &TaskRecord, repo: &RepoRecord) -> (Vec<String>, Vec<String>) {
    if !repo.is_git {
        return (Vec::new(), Vec::new());
    }
    let git = Git::new(&task.worktree_path);
    let changed = git
        .run(&[
            "merge-base",
            "HEAD",
            &format!("refs/heads/{}", repo.default_branch),
        ])
        .ok()
        .and_then(|base| changed_paths(&git, &base).ok())
        .unwrap_or_default();
    let status = git
        .run_text(&["status", "--porcelain"])
        .map(|text| text.lines().map(str::to_owned).collect())
        .unwrap_or_default();
    (changed, status)
}

/// The editable summary of a task, to review before it goes anywhere.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffSummary {
    pub text: String,
    pub max_bytes: usize,
}

/// Builds the summary from the task's first prompt, changed files, git status, last QA run and
/// unanswered questions. Nothing is sent.
#[tauri::command]
pub async fn task_handoff_summary(
    database: State<'_, Database>,
    task_id: String,
) -> Result<HandoffSummary, String> {
    let (task, repo) = {
        let connection = database.lock()?;
        let task = TaskRecord::get(&connection, &task_id).map_err(|e| e.to_string())?;
        let repo = RepoRecord::get(&connection, &task.repo_id).map_err(|e| e.to_string())?;
        (task, repo)
    };
    let (changed, status_lines) = {
        let (task, repo) = (task.clone(), repo.clone());
        tauri::async_runtime::spawn_blocking(move || read_git(&task, &repo))
            .await
            .map_err(|e| e.to_string())?
    };
    let connection = database.lock()?;
    let qa = latest_qa_result(&connection, &task.id)
        .map_err(|e| e.to_string())?
        .map(|run| (run.outcome, run.command, run.verified_by_brachio));
    let input = SummaryInput {
        title: task.title.clone(),
        branch: task.branch.clone(),
        goal: task.prompt.clone(),
        changed,
        status_lines,
        qa,
        questions: open_questions(&connection, &task.id),
    };
    Ok(HandoffSummary {
        text: render(&input),
        max_bytes: SUMMARY_MAX_BYTES,
    })
}

/// Sends the text the user reviewed to an agent tab as a `handoff` message. It waits in that
/// agent's mailbox like any message and carries no approval.
pub(crate) fn mail_summary(
    connection: &Connection,
    from_task_id: &str,
    to_key: &str,
    body: &str,
    id: &str,
    request_id: Option<&str>,
    now_ms: i64,
) -> Result<(), String> {
    let from = TaskRecord::get(connection, from_task_id).map_err(|e| e.to_string())?;
    let target = agent_owner(connection, to_key)?;
    let from_key = format!("{}:agent", from.id);
    if from_key == to_key {
        return Err("Choose another agent than the task the summary comes from.".to_owned());
    }
    if body.trim().is_empty() {
        return Err("The summary is empty.".to_owned());
    }
    if body.len() > SUMMARY_MAX_BYTES.min(MAX_BODY_BYTES) {
        return Err(format!(
            "The summary is longer than {SUMMARY_MAX_BYTES} bytes; shorten it."
        ));
    }
    send_message(
        connection,
        &NewMessage {
            id,
            from_key: &from_key,
            to_key,
            kind: "handoff",
            priority: "normal",
            subject: &format!("Context from \"{}\" for \"{}\"", from.title, target.title)
                .chars()
                .filter(|c| !c.is_control())
                .take(200)
                .collect::<String>(),
            body,
            payload_json: None,
            reply_to: None,
            request_id,
            created_at_ms: now_ms,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// The user's decision to send the reviewed summary to an agent tab's mailbox.
#[tauri::command]
pub fn task_handoff_send(
    app: AppHandle,
    database: State<'_, Database>,
    from_task_id: String,
    to_key: String,
    body: String,
    request_id: Option<String>,
) -> Result<(), String> {
    {
        let connection = database.lock()?;
        mail_summary(
            &connection,
            &from_task_id,
            &to_key,
            &body,
            &uuid::Uuid::new_v4().hyphenated().to_string(),
            request_id.as_deref().filter(|id| !id.trim().is_empty()),
            crate::pr_status::now_ms(),
        )?;
    }
    if let Err(error) = app.emit(crate::mailbox::CHANGED_EVENT, ()) {
        tracing::debug!(%error, "could not tell the window about the handoff message");
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn input() -> SummaryInput {
        SummaryInput {
            title: "Add login".to_owned(),
            branch: "brachio/login".to_owned(),
            goal: "Add a login form".to_owned(),
            changed: vec!["src/login.rs".to_owned()],
            status_lines: vec![" M src/login.rs".to_owned()],
            qa: Some(("failed".to_owned(), "cargo test".to_owned(), true)),
            questions: vec!["Which provider?".to_owned()],
        }
    }

    #[test]
    fn the_summary_names_goal_files_status_qa_and_open_questions() {
        let text = render(&input());
        assert!(text.contains("## Goal\nAdd a login form"), "{text}");
        assert!(text.contains("## Changed files\n- src/login.rs"), "{text}");
        assert!(text.contains("## Git status\n-  M src/login.rs"), "{text}");
        assert!(
            text.contains("failed: cargo test (run by Brachio)"),
            "{text}"
        );
        assert!(
            text.contains("## Open questions\n- Which provider?"),
            "{text}"
        );
    }

    #[test]
    fn a_huge_task_stays_under_the_cap_even_with_multibyte_text() {
        let mut big = input();
        big.goal = "목표".repeat(5000);
        big.changed = (0..500).map(|n| format!("path/to/file-{n}.rs")).collect();
        big.status_lines = (0..500).map(|n| format!(" M file-{n}.rs")).collect();
        big.questions = (0..50)
            .map(|n| "질문".repeat(100) + &n.to_string())
            .collect();
        let text = render(&big);
        assert!(text.len() <= SUMMARY_MAX_BYTES + 4, "{}", text.len());
        assert!(text.contains("... and 475 more files"), "{text}");
        assert!(text.contains("... and 46 more questions"), "{text}");
    }

    fn db() -> Connection {
        let connection = brachio_store::open_in_memory().unwrap();
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('a', 'r', 'A', 'p', 'a', '/w', 'new', 0), ('b', 'r', 'B', 'p', 'b', '/w', 'new', 0);",
            )
            .unwrap();
        connection
    }

    #[test]
    fn mailing_needs_another_agent_tab_and_a_summary_within_the_cap() {
        let db = db();
        assert!(mail_summary(&db, "a", "a:agent", "text", "m0", None, 1).is_err());
        assert!(mail_summary(&db, "a", "b:agent", "  ", "m1", None, 1).is_err());
        let too_long = "x".repeat(SUMMARY_MAX_BYTES + 1);
        assert!(mail_summary(&db, "a", "b:agent", &too_long, "m2", None, 1).is_err());
        mail_summary(&db, "a", "b:agent", "the summary", "m3", None, 1).unwrap();
        let waiting = brachio_store::peek_inbox(&db, "b:agent", 10).unwrap();
        assert_eq!(waiting.len(), 1);
        assert_eq!(waiting[0].kind, "handoff");
        assert_eq!(waiting[0].body, "the summary");
    }

    #[test]
    fn sending_the_same_request_twice_leaves_one_message() {
        let db = db();
        mail_summary(&db, "a", "b:agent", "the summary", "m1", Some("req"), 1).unwrap();
        mail_summary(&db, "a", "b:agent", "the summary", "m2", Some("req"), 2).unwrap();
        assert_eq!(
            brachio_store::peek_inbox(&db, "b:agent", 10).unwrap().len(),
            1
        );
    }
}
