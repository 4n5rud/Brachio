//! A lead task's view of its child tasks, how a child hands work over, and how the user reviews
//! it. Rules live in `brachio_core::task_graph` and `brachio_store`; this module reads the
//! repository and turns the user's decision into a stored state change.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use brachio_core::task_graph::TaskState;
use brachio_git::Git;
use brachio_runtime::mailbox::MailResponse;
use brachio_store::{
    decide_handoff, decide_handoff_then, fail_lost, load_graph, pending_gates, qa_result_by_id,
    record_start_failure, reopen_failed, running_children, schedule_next, scheduled_leads,
    scheduler_enabled, send_message, set_scheduler_enabled, start_circuit_broken, state_reason,
    submit_handoff, Handoff, HandoffReview, HandoffSource, NewHandoff, NewMessage, RepoRecord,
    TaskRecord, LOST_AFTER_RESTART, MAX_RUNNING_CHILDREN,
};
use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::child_task_request::is_hidden_or_control;
use crate::mailbox::agent_owner;
use crate::runtime_terminals::RuntimeTerminals;
use crate::terminal::Terminals;
use crate::workspace::{live_task_terminals, CreatedTask, Database};

/// Tells the interface that a child's state or handoff changed.
pub const CHANGED_EVENT: &str = "orchestration://changed";
/// Tells the interface to open a task the scheduler just started. The payload is a
/// `workspace::CreatedTask`, like `task://created`.
pub const STARTED_EVENT: &str = "task://started";
const MAX_SUMMARY_CHARS: usize = 4000;
const MAX_TESTS: usize = 20;
const MAX_TEST_CHARS: usize = 200;
const MAX_NOTE_CHARS: usize = 300;
const MAX_SHOWN_PATHS: usize = 200;

/// A lead's children as the Tasks tab shows them.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestrationView {
    pub lead_task_id: String,
    /// Ready children start by themselves, at most `max_running` at a time.
    pub scheduler_enabled: bool,
    pub max_running: usize,
    pub children: Vec<ChildView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildView {
    pub id: String,
    pub title: String,
    pub branch: String,
    pub state: String,
    /// Why Brachio failed it, such as "lost after restart".
    pub reason: Option<String>,
    /// It failed to start too many times; only the user can reopen it.
    pub start_blocked: bool,
    pub dependencies: Vec<DependencyView>,
    pub handoff: Option<HandoffView>,
    /// Decisions the child asked the user to make; they hold back what depends on it.
    pub gates: Vec<GateView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GateView {
    pub id: String,
    /// The child's words; untrusted.
    pub subject: String,
    pub body: String,
    pub options: Vec<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyView {
    pub id: String,
    pub title: String,
    pub state: String,
}

/// What an agent claimed and, apart from it, what Brachio verified.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffView {
    pub id: String,
    pub commit_sha: String,
    pub base_sha: String,
    /// Agent's words; untrusted.
    pub summary: String,
    pub changed_paths_count: usize,
    pub changed_paths: Vec<String>,
    /// Tests the agent says it ran; untrusted.
    pub reported_tests: Vec<String>,
    /// A QA run Brachio executed on exactly this commit.
    pub verified_qa: Option<VerifiedQaView>,
    /// `approved` or `rejected` once the user decided.
    pub review: Option<String>,
    pub review_note: Option<String>,
    pub created_at_ms: i64,
    /// The task waits for the user's decision on this handoff.
    pub can_review: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedQaView {
    pub outcome: String,
    pub command: String,
    pub exit_code: Option<i32>,
    pub duration_ms: i64,
}

/// Reads the lead's children, their dependencies and latest handoffs.
pub fn build_view(connection: &Connection, lead_id: &str) -> Result<OrchestrationView, String> {
    let graph = load_graph(connection, lead_id).map_err(|e| e.to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT id, title, branch, archived_at_ms IS NOT NULL FROM tasks
             WHERE parent_task_id = ?1 ORDER BY created_at_ms, id",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, String, String, bool)> = statement
        .query_map([lead_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .and_then(Iterator::collect)
        .map_err(|e| e.to_string())?;
    let title_of = |id: &str| {
        rows.iter()
            .find(|row| row.0 == id)
            .map_or_else(|| id.to_owned(), |row| row.1.clone())
    };
    let mut children = Vec::new();
    for (id, title, branch, archived) in &rows {
        if *archived {
            continue;
        }
        let state = graph.state(id).map_err(|e| e.to_string())?;
        let dependencies = graph
            .dependencies_of(id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|dependency| DependencyView {
                id: dependency.to_owned(),
                title: title_of(dependency),
                state: graph
                    .state(dependency)
                    .map_or_else(|_| "unknown".to_owned(), |s| s.as_str().to_owned()),
            })
            .collect();
        let handoff = Handoff::latest(connection, id)
            .map_err(|e| e.to_string())?
            .map(|handoff| handoff_view(connection, handoff, state))
            .transpose()?;
        children.push(ChildView {
            id: id.clone(),
            title: title.clone(),
            branch: branch.clone(),
            state: state.as_str().to_owned(),
            reason: state_reason(connection, id).map_err(|e| e.to_string())?,
            start_blocked: start_circuit_broken(connection, id).map_err(|e| e.to_string())?,
            dependencies,
            handoff,
            gates: pending_gates(connection, id)
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|gate| GateView {
                    id: gate.message_id,
                    subject: gate.subject,
                    body: gate.body,
                    options: gate.options,
                    created_at_ms: gate.created_at_ms,
                })
                .collect(),
        });
    }
    Ok(OrchestrationView {
        lead_task_id: lead_id.to_owned(),
        scheduler_enabled: scheduler_enabled(connection, lead_id).map_err(|e| e.to_string())?,
        max_running: MAX_RUNNING_CHILDREN,
        children,
    })
}

fn handoff_view(
    connection: &Connection,
    handoff: Handoff,
    state: TaskState,
) -> Result<HandoffView, String> {
    let verified_qa = match handoff.verified_qa_result_id {
        Some(id) => qa_result_by_id(connection, id)
            .map_err(|e| e.to_string())?
            .map(|qa| VerifiedQaView {
                outcome: qa.outcome,
                command: qa.command,
                exit_code: qa.exit_code,
                duration_ms: qa.duration_ms,
            }),
        None => None,
    };
    Ok(HandoffView {
        can_review: state == TaskState::ReviewRequired && handoff.review.is_none(),
        id: handoff.id,
        commit_sha: handoff.commit_sha,
        base_sha: handoff.base_sha,
        summary: handoff.summary,
        changed_paths_count: handoff.changed_paths.len(),
        changed_paths: handoff
            .changed_paths
            .into_iter()
            .take(MAX_SHOWN_PATHS)
            .collect(),
        reported_tests: handoff.reported_tests,
        verified_qa,
        review: handoff.review.map(|review| {
            match review {
                HandoffReview::Approved => "approved",
                HandoffReview::Rejected => "rejected",
            }
            .to_owned()
        }),
        review_note: handoff.review_note,
        created_at_ms: handoff.created_at_ms,
    })
}

fn clean_text(label: &str, text: &str, max_chars: usize) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > max_chars {
        return Err(format!("{label} must be 1 to {max_chars} characters."));
    }
    if text.chars().any(is_hidden_or_control) {
        return Err(format!(
            "{label} contains control or invisible characters, which are not allowed."
        ));
    }
    Ok(text.to_owned())
}

/// Records the handoff of the asking child: the app, not the agent, reads the commit, the base
/// and the changed files from the child's worktree.
pub fn handoff_request(
    connection: &mut Connection,
    key: &str,
    summary: &str,
    tests: &[String],
    now_ms: i64,
    handoff_id: &str,
) -> Result<MailResponse, String> {
    let child = agent_owner(connection, key)?;
    let lead_id = child
        .parent_task_id
        .clone()
        .ok_or_else(|| "Only a child task can hand off work.".to_owned())?;
    let lead = TaskRecord::get(connection, &lead_id).map_err(|e| e.to_string())?;
    if !RepoRecord::get(connection, &child.repo_id)
        .map_err(|e| e.to_string())?
        .is_git
    {
        return Err("Handoffs need a git project.".to_owned());
    }
    let summary = clean_text("The summary", summary, MAX_SUMMARY_CHARS)?;
    if tests.len() > MAX_TESTS {
        return Err(format!("At most {MAX_TESTS} tests can be listed."));
    }
    let tests = tests
        .iter()
        .map(|test| clean_text("A test name", test, MAX_TEST_CHARS))
        .collect::<Result<Vec<_>, _>>()?;
    let snapshot = read_commit(&child.worktree_path, &lead.branch)?;
    submit_handoff(
        connection,
        &NewHandoff {
            id: handoff_id,
            task_id: &child.id,
            commit_sha: &snapshot.head,
            base_sha: &snapshot.base,
            summary: &summary,
            changed_paths: &snapshot.changed,
            reported_tests: &tests,
            source: HandoffSource::ReportedByAgent,
            created_at_ms: now_ms,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(MailResponse::HandedOff {
        commit_sha: snapshot.head,
        changed_paths: snapshot.changed.len(),
    })
}

struct CommitSnapshot {
    head: String,
    base: String,
    changed: Vec<String>,
}

fn read_commit(worktree: &Path, lead_branch: &str) -> Result<CommitSnapshot, String> {
    let git = Git::new(worktree);
    let dirty = git
        .run_text(&["status", "--porcelain"])
        .map_err(|e| e.to_string())?;
    if !dirty.trim().is_empty() {
        return Err("Commit your work first: the worktree has uncommitted changes.".to_owned());
    }
    let head = git.run(&["rev-parse", "HEAD"]).map_err(|e| e.to_string())?;
    let base = git
        .run(&["merge-base", "HEAD", lead_branch])
        .map_err(|e| e.to_string())?;
    if head == base {
        return Err("There are no commits on this branch to hand off.".to_owned());
    }
    let changed = git
        .run_text(&["diff", "--name-only", "-z", &base, &head])
        .map_err(|e| e.to_string())?
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect();
    Ok(CommitSnapshot {
        head,
        base,
        changed,
    })
}

/// The commit currently checked out in `worktree`.
pub fn worktree_head(worktree: &Path) -> Result<String, String> {
    Git::new(worktree)
        .run(&["rev-parse", "HEAD"])
        .map_err(|e| e.to_string())
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(8)]
}

/// Applies the user's decision. Approving needs the branch to still be at the handed-off
/// commit; rejecting needs a reason, which is mailed to the child's agent in the same
/// transaction, so a rejection that cannot be delivered is not recorded.
pub fn review_decision(
    connection: &mut Connection,
    handoff_id: &str,
    approved: bool,
    note: Option<&str>,
    head_of: impl Fn(&Path) -> Result<String, String>,
    now_ms: i64,
    message_id: &str,
) -> Result<(), String> {
    let handoff = Handoff::get(connection, handoff_id).map_err(|e| e.to_string())?;
    let task = TaskRecord::get(connection, &handoff.task_id).map_err(|e| e.to_string())?;
    if approved {
        if !pending_gates(connection, &task.id)
            .map_err(|e| e.to_string())?
            .is_empty()
        {
            return Err(
                "This task has an unanswered decision. Answer it before approving.".to_owned(),
            );
        }
        let head = head_of(&task.worktree_path)?;
        if head != handoff.commit_sha {
            return Err(format!(
                "The branch moved after the handoff (handed off {}, now {}). Nothing was approved; ask the agent to hand off again.",
                short(&handoff.commit_sha),
                short(&head)
            ));
        }
        return decide_handoff(
            connection,
            handoff_id,
            HandoffReview::Approved,
            None,
            now_ms,
        )
        .map_err(|e| e.to_string());
    }
    let note = clean_text("The reason", note.unwrap_or(""), MAX_NOTE_CHARS)?;
    let lead_id = task.parent_task_id.as_deref();
    decide_handoff_then(
        connection,
        handoff_id,
        HandoffReview::Rejected,
        Some(&note),
        now_ms,
        |transaction| {
            let Some(lead_id) = lead_id else {
                return Ok(());
            };
            send_message(
                transaction,
                &NewMessage {
                    id: message_id,
                    from_key: &format!("{lead_id}:agent"),
                    to_key: &format!("{}:agent", task.id),
                    kind: "status",
                    priority: "high",
                    subject: "Review rejected: rework needed",
                    body: &note,
                    payload_json: None,
                    reply_to: None,
                    request_id: None,
                    created_at_ms: now_ms,
                },
            )?;
            Ok(())
        },
    )
    .map_err(|e| {
        format!("The rejection could not be delivered to the agent, so nothing was changed: {e}")
    })
}

/// Starts what the schedulers allow and tells the window to open those tasks. Each child is
/// started by exactly one call, because the state change and the limit are checked in one
/// transaction.
pub fn schedule_all<R: Runtime>(app: &AppHandle<R>) {
    let database = app.state::<Database>();
    let started = match start_ready_children(&database) {
        Ok(started) => started,
        Err(error) => {
            tracing::warn!(%error, "could not schedule child tasks");
            return;
        }
    };
    if started.is_empty() {
        return;
    }
    for task in started {
        if let Err(error) = app.emit(STARTED_EVENT, &task) {
            tracing::debug!(%error, "could not tell the window about a started task");
        }
    }
    if let Err(error) = app.emit(CHANGED_EVENT, ()) {
        tracing::debug!(%error, "could not tell the window about started tasks");
    }
}

fn start_ready_children(database: &Database) -> Result<Vec<CreatedTask>, String> {
    let mut connection = database.lock()?;
    start_children(&mut connection, crate::pr_status::now_ms(), |repo| {
        brachio_git::read_setup(repo)
            .map(|setup| setup.run)
            .map_err(|e| e.to_string())
    })
}

/// A child whose setup cannot be read is not launched as if it had no setup. The start is
/// retried, and after `MAX_START_FAILURES` failures the child stays failed (blocking its
/// dependents) until the user reopens it.
fn start_children(
    connection: &mut Connection,
    now: i64,
    setup_of: impl Fn(&Path) -> Result<Vec<String>, String>,
) -> Result<Vec<CreatedTask>, String> {
    let mut started = Vec::new();
    for lead in scheduled_leads(connection).map_err(|e| e.to_string())? {
        for id in schedule_next(connection, &lead, now).map_err(|e| e.to_string())? {
            let task = TaskRecord::get(connection, &id).map_err(|e| e.to_string())?;
            let repo = RepoRecord::get(connection, &task.repo_id).map_err(|e| e.to_string())?;
            match setup_of(&repo.path) {
                Ok(setup) => started.push(CreatedTask::started(task, setup)),
                Err(error) => {
                    let reason = format!("invalid setup configuration: {error}");
                    tracing::warn!(task = %id, %reason, "child task not started");
                    record_start_failure(connection, &id, &reason, now)
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(started)
}

/// After a restart, children recorded as running whose terminal is not hosted any more are
/// failed ("lost after restart"), which blocks their dependents. A terminal the runtime still
/// hosts, or one that could not be checked, is left running. Nothing is started or replayed.
pub async fn reconcile_after_restart(app: &AppHandle) {
    let database = app.state::<Database>();
    let running = match database
        .lock()
        .and_then(|c| running_children(&c).map_err(|e| e.to_string()))
    {
        Ok(running) => running,
        Err(error) => {
            tracing::warn!(%error, "could not read running child tasks");
            return;
        }
    };
    let terminals = app.state::<RuntimeTerminals>();
    let in_app = app.state::<Terminals>();
    let mut lost = Vec::new();
    for id in running {
        match live_task_terminals(&database, &terminals, &in_app, &id).await {
            Ok(live) if live.is_empty() => lost.push(id),
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, task = %id, "could not tell if a child is still running");
            }
        }
    }
    if lost.is_empty() {
        return;
    }
    let failed = database.lock().and_then(|mut c| {
        fail_lost(&mut c, &lost, crate::pr_status::now_ms()).map_err(|e| e.to_string())
    });
    match failed {
        Ok(failed) => {
            tracing::info!(
                tasks = failed.len(),
                reason = LOST_AFTER_RESTART,
                "failed lost child tasks"
            );
        }
        Err(error) => tracing::warn!(%error, "could not fail lost child tasks"),
    }
}

/// Turns the lead's scheduler on or off, and starts what is ready when it goes on.
#[tauri::command]
pub fn orchestration_set_scheduler(
    app: AppHandle,
    database: State<'_, Database>,
    lead_task_id: String,
    enabled: bool,
) -> Result<(), String> {
    {
        let connection = database.lock()?;
        let lead = TaskRecord::get(&connection, &lead_task_id).map_err(|e| e.to_string())?;
        if lead.parent_task_id.is_some() {
            return Err("Only a task that is not a child can schedule child tasks.".to_owned());
        }
        set_scheduler_enabled(&connection, &lead_task_id, enabled).map_err(|e| e.to_string())?;
    }
    schedule_all(&app);
    if let Err(error) = app.emit(CHANGED_EVENT, ()) {
        tracing::debug!(%error, "could not tell the window about the scheduler change");
    }
    Ok(())
}

/// The user's decision to try a child that stopped after repeated start failures again.
#[tauri::command]
pub fn orchestration_reopen_child(
    app: AppHandle,
    database: State<'_, Database>,
    task_id: String,
) -> Result<(), String> {
    {
        let mut connection = database.lock()?;
        reopen_failed(&mut connection, &task_id, crate::pr_status::now_ms())
            .map_err(|e| e.to_string())?;
    }
    schedule_all(&app);
    if let Err(error) = app.emit(CHANGED_EVENT, ()) {
        tracing::debug!(%error, "could not tell the window about the reopened task");
    }
    Ok(())
}

/// Run by the window once it listens: the first call reconciles children left running by a
/// previous run, then every call starts ready children of the leads that opted in. Reconciling
/// first keeps lost children from taking up the running limit.
#[tauri::command]
pub async fn orchestration_schedule(app: AppHandle) {
    static RECONCILED: AtomicBool = AtomicBool::new(false);
    if !RECONCILED.swap(true, Ordering::SeqCst) {
        reconcile_after_restart(&app).await;
    }
    schedule_all(&app);
}

/// The lead's children with state, dependencies and latest handoff.
#[tauri::command]
pub fn orchestration_view(
    database: State<'_, Database>,
    lead_task_id: String,
) -> Result<OrchestrationView, String> {
    build_view(&*database.lock()?, &lead_task_id)
}

/// Decides under the database lock, reading the worktree's HEAD at decision time so a branch
/// that moved after the review began is still caught.
fn review_handoff(
    database: &Database,
    handoff_id: &str,
    approved: bool,
    note: Option<&str>,
    head_of: impl Fn(&Path) -> Result<String, String>,
) -> Result<(), String> {
    let mut connection = database.lock()?;
    review_decision(
        &mut connection,
        handoff_id,
        approved,
        note,
        head_of,
        crate::pr_status::now_ms(),
        &uuid::Uuid::new_v4().hyphenated().to_string(),
    )
}

/// The user's decision on a handoff. Approving completes the child, rejecting sends it back to
/// work with the reason. Nothing here merges anything.
#[tauri::command]
pub fn handoff_review(
    app: AppHandle,
    database: State<'_, Database>,
    handoff_id: String,
    approved: bool,
    note: Option<String>,
) -> Result<(), String> {
    review_handoff(
        &database,
        &handoff_id,
        approved,
        note.as_deref(),
        worktree_head,
    )?;
    schedule_all(&app);
    if let Err(error) = app.emit(CHANGED_EVENT, ()) {
        tracing::debug!(%error, "could not tell the window about the review");
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use brachio_runtime::mailbox::MailResponse;
    use brachio_store::open_in_memory;

    use super::{
        build_view, handoff_request, review_decision, review_handoff, start_children, worktree_head,
    };
    use crate::workspace::Database;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    /// A repository on branch `child` with one commit past `main`, plus a database that holds
    /// a lead on `main` and the child in that directory.
    fn setup() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-q", "-m", "init"]);
        git(dir.path(), &["checkout", "-q", "-b", "child"]);
        std::fs::write(dir.path().join("b.txt"), "b").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-q", "-m", "work"]);
        let db = open_in_memory().unwrap();
        let path = dir.path().to_str().unwrap();
        db.execute(
            "INSERT INTO repos (id, path, default_branch) VALUES ('r', ?1, 'main')",
            [path],
        )
        .unwrap();
        db.execute(
            "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
             VALUES ('lead', 'r', 'Lead', '', 'main', ?1, 'new', 0)",
            [path],
        )
        .unwrap();
        db.execute(
            "INSERT INTO tasks (id, repo_id, parent_task_id, title, prompt, branch, worktree_path, state, created_at_ms)
             VALUES ('child', 'r', 'lead', 'Child', '', 'child', ?1, 'new', 1)",
            [path],
        )
        .unwrap();
        (dir, db)
    }

    #[test]
    fn a_handoff_records_the_commit_and_waits_for_review() {
        let (dir, mut db) = setup();
        let response = handoff_request(
            &mut db,
            "child:agent",
            "added b",
            &["cargo test".to_owned()],
            5,
            "h1",
        )
        .unwrap();
        let head = worktree_head(dir.path()).unwrap();
        assert_eq!(
            response,
            MailResponse::HandedOff {
                commit_sha: head.clone(),
                changed_paths: 1
            }
        );
        let view = build_view(&db, "lead").unwrap();
        let child = &view.children[0];
        assert_eq!(child.state, "review_required");
        let handoff = child.handoff.as_ref().unwrap();
        assert_eq!(handoff.commit_sha, head);
        assert_eq!(handoff.changed_paths, vec!["b.txt".to_owned()]);
        assert_eq!(handoff.reported_tests, vec!["cargo test".to_owned()]);
        assert!(handoff.verified_qa.is_none() && handoff.can_review);
    }

    #[test]
    fn uncommitted_work_and_a_lead_are_refused() {
        let (dir, mut db) = setup();
        std::fs::write(dir.path().join("c.txt"), "c").unwrap();
        let dirty = handoff_request(&mut db, "child:agent", "s", &[], 5, "h1").unwrap_err();
        assert!(dirty.contains("uncommitted"), "{dirty}");
        let lead = handoff_request(&mut db, "lead:agent", "s", &[], 5, "h2").unwrap_err();
        assert!(lead.contains("child"), "{lead}");
    }

    #[test]
    fn approving_a_moved_branch_is_refused_and_changes_nothing() {
        let (dir, mut db) = setup();
        handoff_request(&mut db, "child:agent", "added b", &[], 5, "h1").unwrap();
        std::fs::write(dir.path().join("d.txt"), "d").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-q", "-m", "late"]);
        let error =
            review_decision(&mut db, "h1", true, None, worktree_head_of, 9, "m1").unwrap_err();
        assert!(error.contains("moved after the handoff"), "{error}");
        assert_eq!(
            build_view(&db, "lead").unwrap().children[0].state,
            "review_required"
        );
    }

    fn worktree_head_of(path: &Path) -> Result<String, String> {
        worktree_head(path)
    }

    #[test]
    fn approving_completes_and_rejecting_needs_a_reason() {
        let (_dir, mut db) = setup();
        handoff_request(&mut db, "child:agent", "added b", &[], 5, "h1").unwrap();
        let blank = review_decision(&mut db, "h1", false, Some("  "), worktree_head_of, 6, "m1");
        assert!(blank.is_err());
        review_decision(
            &mut db,
            "h1",
            false,
            Some("add tests"),
            worktree_head_of,
            7,
            "m1",
        )
        .unwrap();
        assert_eq!(
            build_view(&db, "lead").unwrap().children[0].state,
            "running"
        );
        handoff_request(&mut db, "child:agent", "added tests", &[], 8, "h2").unwrap();
        review_decision(&mut db, "h2", true, None, worktree_head_of, 9, "m2").unwrap();
        assert_eq!(
            build_view(&db, "lead").unwrap().children[0].state,
            "completed"
        );
    }

    #[test]
    fn a_dependent_starts_only_after_the_exact_commit_is_approved() {
        let (dir, mut db) = setup();
        db.execute(
            "INSERT INTO tasks (id, repo_id, parent_task_id, title, prompt, branch, worktree_path, state, created_at_ms)
             VALUES ('next', 'r', 'lead', 'Next', '', 'next', '/elsewhere', 'new', 2)",
            [],
        )
        .unwrap();
        brachio_store::add_dependency(&mut db, "next", "child").unwrap();
        brachio_store::set_scheduler_enabled(&db, "lead", true).unwrap();
        assert_eq!(
            brachio_store::schedule_next(&mut db, "lead", 1).unwrap(),
            vec!["child"]
        );

        handoff_request(&mut db, "child:agent", "added b", &[], 2, "h1").unwrap();
        assert!(brachio_store::schedule_next(&mut db, "lead", 3)
            .unwrap()
            .is_empty());

        std::fs::write(dir.path().join("late.txt"), "x").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-q", "-m", "late"]);
        let stale = review_decision(&mut db, "h1", true, None, worktree_head_of, 4, "m1");
        assert!(stale.unwrap_err().contains("moved after the handoff"));
        assert!(brachio_store::schedule_next(&mut db, "lead", 5)
            .unwrap()
            .is_empty());

        handoff_request(&mut db, "child:agent", "added b and late", &[], 6, "h2").unwrap();
        review_decision(&mut db, "h2", true, None, worktree_head_of, 7, "m2").unwrap();
        assert_eq!(
            brachio_store::schedule_next(&mut db, "lead", 8).unwrap(),
            vec!["next"]
        );
    }

    #[test]
    fn approval_reads_head_after_the_review_began() {
        let (dir, mut db) = setup();
        handoff_request(&mut db, "child:agent", "added b", &[], 5, "h1").unwrap();
        let database = Database(std::sync::Mutex::new(db));
        let error = review_handoff(&database, "h1", true, None, |path| {
            std::fs::write(path.join("race.txt"), "x").unwrap();
            git(path, &["add", "."]);
            git(path, &["commit", "-q", "-m", "race"]);
            worktree_head(path)
        })
        .unwrap_err();
        assert!(error.contains("moved after the handoff"), "{error}");
        let db = database.0.lock().unwrap();
        assert_eq!(
            build_view(&db, "lead").unwrap().children[0].state,
            "review_required"
        );
        drop(dir);
    }

    #[test]
    fn a_rejection_that_cannot_be_mailed_is_not_recorded() {
        let (_dir, mut db) = setup();
        handoff_request(&mut db, "child:agent", "added b", &[], 5, "h1").unwrap();
        for n in 0..brachio_store::MAX_UNREAD_FROM_ONE_SENDER {
            brachio_store::send_message(
                &db,
                &brachio_store::NewMessage {
                    id: &format!("fill{n}"),
                    from_key: "lead:agent",
                    to_key: "child:agent",
                    kind: "status",
                    priority: "normal",
                    subject: "s",
                    body: "b",
                    payload_json: None,
                    reply_to: None,
                    request_id: None,
                    created_at_ms: 6,
                },
            )
            .unwrap();
        }
        let error = review_decision(
            &mut db,
            "h1",
            false,
            Some("fix it"),
            worktree_head_of,
            7,
            "m1",
        )
        .unwrap_err();
        assert!(error.contains("could not be delivered"), "{error}");
        assert_eq!(
            build_view(&db, "lead").unwrap().children[0].state,
            "review_required"
        );
    }

    #[test]
    fn a_child_with_invalid_setup_fails_instead_of_starting_with_none() {
        let (_dir, mut db) = setup();
        brachio_store::set_scheduler_enabled(&db, "lead", true).unwrap();
        let bad = |_: &Path| Err("bad brachio.toml".to_owned());
        for pass in 1..brachio_store::MAX_START_FAILURES {
            assert!(start_children(&mut db, 1, bad).unwrap().is_empty());
            let view = build_view(&db, "lead").unwrap();
            assert_eq!(view.children[0].state, "queued", "pass {pass}");
            assert!(!view.children[0].start_blocked);
        }
        assert!(start_children(&mut db, 1, bad).unwrap().is_empty());
        let view = build_view(&db, "lead").unwrap();
        assert_eq!(view.children[0].state, "failed");
        assert!(view.children[0].start_blocked);
        let reason = brachio_store::state_reason(&db, "child").unwrap();
        assert!(reason.is_some_and(|r| r.contains("invalid setup configuration: bad brachio.toml")));
        assert!(start_children(&mut db, 2, bad).unwrap().is_empty());
    }
}
