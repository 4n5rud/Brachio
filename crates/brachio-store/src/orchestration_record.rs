//! Persisted dependency graph, task states and reviewed handoffs for a lead's child tasks.
//! Rules live in `brachio_core::task_graph`; this module loads the graph, asks it, and
//! writes the answer and every consequence in one transaction.

use brachio_core::task_graph::{Change, GraphError, TaskGraph, TaskState};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::StoreError;

/// Why an orchestration call was refused.
#[derive(Debug, thiserror::Error)]
pub enum OrchestrationError {
    /// The graph model rejected the change.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// Completing needs an approved review of the task's latest handoff.
    #[error("task {task_id} has no approved handoff review")]
    HandoffNotApproved {
        /// The task that cannot complete.
        task_id: String,
    },
    /// A commit id is not 40 or 64 lowercase hex characters.
    #[error("not a full lowercase commit sha: {0:?}")]
    InvalidCommitSha(String),
    /// The task is not a child of a lead task.
    #[error("task {0} is not a child task")]
    NotAChild(String),
    /// The handoff was already approved or rejected.
    #[error("handoff {0} was already reviewed")]
    AlreadyReviewed(String),
    /// A stored value is not understood.
    #[error("{0}")]
    Corrupt(String),
    /// The database failed or a row is missing.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<rusqlite::Error> for OrchestrationError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

/// Who produced a handoff's contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffSource {
    /// An agent's own claim; untrusted.
    ReportedByAgent,
    /// Recomputed by Brachio from the repository.
    VerifiedByBrachio,
}

impl HandoffSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::ReportedByAgent => "reported_by_agent",
            Self::VerifiedByBrachio => "verified_by_brachio",
        }
    }

    fn parse(text: &str) -> Result<Self, OrchestrationError> {
        match text {
            "reported_by_agent" => Ok(Self::ReportedByAgent),
            "verified_by_brachio" => Ok(Self::VerifiedByBrachio),
            other => Err(OrchestrationError::Corrupt(format!(
                "unknown handoff source: {other}"
            ))),
        }
    }
}

/// A human's decision on a handoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffReview {
    /// The exact commit was accepted.
    Approved,
    /// The commit was turned down.
    Rejected,
}

impl HandoffReview {
    fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }

    fn parse(text: &str) -> Result<Self, OrchestrationError> {
        match text {
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            other => Err(OrchestrationError::Corrupt(format!(
                "unknown handoff review: {other}"
            ))),
        }
    }
}

/// A handoff to store.
#[derive(Debug, Clone)]
pub struct NewHandoff<'a> {
    /// Identifier of the handoff.
    pub id: &'a str,
    /// The task handing off.
    pub task_id: &'a str,
    /// Exact commit being handed off.
    pub commit_sha: &'a str,
    /// Exact commit the diff is measured against.
    pub base_sha: &'a str,
    /// Agent's summary; untrusted text.
    pub summary: &'a str,
    /// Paths the work changed.
    pub changed_paths: &'a [String],
    /// Tests the agent says it ran; untrusted claims.
    pub reported_tests: &'a [String],
    /// Where the contents came from.
    pub source: HandoffSource,
    /// Creation time.
    pub created_at_ms: i64,
}

/// A stored handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handoff {
    /// Identifier.
    pub id: String,
    /// The task handing off.
    pub task_id: String,
    /// Exact commit handed off.
    pub commit_sha: String,
    /// Exact commit the diff is measured against.
    pub base_sha: String,
    /// Agent's summary; untrusted text.
    pub summary: String,
    /// Paths the work changed.
    pub changed_paths: Vec<String>,
    /// Tests the agent says it ran; untrusted claims.
    pub reported_tests: Vec<String>,
    /// Where the contents came from.
    pub source: HandoffSource,
    /// Creation time.
    pub created_at_ms: i64,
    /// When a human decided, if they have.
    pub reviewed_at_ms: Option<i64>,
    /// The decision, if there is one.
    pub review: Option<HandoffReview>,
}

/// Builds the graph of a lead's child tasks from the database.
///
/// A child with no saved state is queued.
///
/// # Errors
/// Fails when the query fails or a stored state is not understood.
pub fn load_graph(
    connection: &Connection,
    lead_task_id: &str,
) -> Result<TaskGraph, OrchestrationError> {
    let mut graph = TaskGraph::new();
    let mut statement = connection.prepare(
        "SELECT t.id, o.state FROM tasks t
         LEFT JOIN task_orchestration o ON o.task_id = t.id
         WHERE t.parent_task_id = ?1 ORDER BY t.created_at_ms, t.id",
    )?;
    let children: Vec<(String, Option<String>)> = statement
        .query_map([lead_task_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (id, _) in &children {
        graph.add_task(id)?;
    }
    let mut statement = connection.prepare(
        "SELECT d.task_id, d.depends_on FROM task_dependencies d
         JOIN tasks t ON t.id = d.task_id
         WHERE t.parent_task_id = ?1 ORDER BY d.rowid",
    )?;
    let edges: Vec<(String, String)> = statement
        .query_map([lead_task_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    // Edges go in while every task is still queued, so saved states cannot lock them out.
    for (task, depends_on) in &edges {
        graph.add_dependency(task, depends_on)?;
    }
    for (id, state) in children {
        if let Some(state) = state {
            let state = state
                .parse::<TaskState>()
                .map_err(|e| OrchestrationError::Corrupt(e.to_string()))?;
            graph.restore_state(&id, state)?;
        }
    }
    Ok(graph)
}

fn lead_of(connection: &Connection, task_id: &str) -> Result<String, OrchestrationError> {
    let row: Option<Option<String>> = connection
        .query_row(
            "SELECT parent_task_id FROM tasks WHERE id = ?1",
            [task_id],
            |row| row.get(0),
        )
        .optional()?;
    match row {
        None => Err(GraphError::UnknownTask(task_id.to_owned()).into()),
        Some(None) => Err(OrchestrationError::NotAChild(task_id.to_owned())),
        Some(Some(lead)) => Ok(lead),
    }
}

/// Makes `task_id` depend on `depends_on`. Both must be children of the same lead; a
/// cycle, an unknown task or a task that already started is refused before anything
/// is written.
///
/// # Errors
/// [`OrchestrationError::Graph`] for refused edges, or a database failure.
pub fn add_dependency(
    connection: &mut Connection,
    task_id: &str,
    depends_on: &str,
) -> Result<(), OrchestrationError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let lead = lead_of(&transaction, task_id)?;
    let mut graph = load_graph(&transaction, &lead)?;
    graph.add_dependency(task_id, depends_on)?;
    transaction.execute(
        "INSERT INTO task_dependencies (task_id, depends_on) VALUES (?1, ?2)",
        [task_id, depends_on],
    )?;
    transaction.commit()?;
    Ok(())
}

/// Moves a task to `to`, saving that change and every task it blocks in one transaction.
/// Completing a task needs an approved review of its latest handoff.
///
/// # Errors
/// [`OrchestrationError::Graph`] for forbidden moves,
/// [`OrchestrationError::HandoffNotApproved`], or a database failure.
pub fn apply_transition(
    connection: &mut Connection,
    task_id: &str,
    to: TaskState,
    now_ms: i64,
) -> Result<Vec<Change>, OrchestrationError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let lead = lead_of(&transaction, task_id)?;
    let mut graph = load_graph(&transaction, &lead)?;
    let changes = graph.transition(task_id, to)?;
    if to == TaskState::Completed && !latest_handoff_approved(&transaction, task_id)? {
        return Err(OrchestrationError::HandoffNotApproved {
            task_id: task_id.to_owned(),
        });
    }
    for change in &changes {
        transaction.execute(
            "INSERT INTO task_orchestration (task_id, state, updated_at_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(task_id) DO UPDATE SET state = excluded.state,
                                                updated_at_ms = excluded.updated_at_ms",
            params![change.id, change.to.as_str(), now_ms],
        )?;
    }
    transaction.commit()?;
    Ok(changes)
}

fn latest_handoff_approved(
    connection: &Connection,
    task_id: &str,
) -> Result<bool, OrchestrationError> {
    let review: Option<Option<String>> = connection
        .query_row(
            "SELECT review FROM task_handoffs WHERE task_id = ?1
             ORDER BY created_at_ms DESC, rowid DESC LIMIT 1",
            [task_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(review.flatten().as_deref() == Some("approved"))
}

fn check_sha(sha: &str) -> Result<(), OrchestrationError> {
    let hex = sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if hex && matches!(sha.len(), 40 | 64) {
        Ok(())
    } else {
        Err(OrchestrationError::InvalidCommitSha(sha.to_owned()))
    }
}

fn to_json(items: &[String]) -> Result<String, OrchestrationError> {
    serde_json::to_string(items).map_err(|e| OrchestrationError::Corrupt(e.to_string()))
}

/// Stores a handoff, unreviewed.
///
/// # Errors
/// [`OrchestrationError::InvalidCommitSha`] for bad commit ids, or a database failure
/// such as an unknown task.
pub fn record_handoff(
    connection: &Connection,
    handoff: &NewHandoff<'_>,
) -> Result<(), OrchestrationError> {
    check_sha(handoff.commit_sha)?;
    check_sha(handoff.base_sha)?;
    connection.execute(
        "INSERT INTO task_handoffs (id, task_id, commit_sha, base_sha, summary, changed_paths,
                                    reported_tests, source, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            handoff.id,
            handoff.task_id,
            handoff.commit_sha,
            handoff.base_sha,
            handoff.summary,
            to_json(handoff.changed_paths)?,
            to_json(handoff.reported_tests)?,
            handoff.source.as_str(),
            handoff.created_at_ms,
        ],
    )?;
    Ok(())
}

/// Records a human's decision on a handoff, once.
///
/// # Errors
/// [`OrchestrationError::AlreadyReviewed`], a missing handoff, or a database failure.
pub fn review_handoff(
    connection: &Connection,
    handoff_id: &str,
    review: HandoffReview,
    now_ms: i64,
) -> Result<(), OrchestrationError> {
    let changed = connection.execute(
        "UPDATE task_handoffs SET review = ?2, reviewed_at_ms = ?3
         WHERE id = ?1 AND review IS NULL",
        params![handoff_id, review.as_str(), now_ms],
    )?;
    if changed == 1 {
        return Ok(());
    }
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM task_handoffs WHERE id = ?1)",
        [handoff_id],
        |row| row.get(0),
    )?;
    if exists {
        Err(OrchestrationError::AlreadyReviewed(handoff_id.to_owned()))
    } else {
        Err(StoreError::NotFound {
            table: "task_handoffs",
            id: handoff_id.to_owned(),
        }
        .into())
    }
}

type HandoffRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    Option<i64>,
    Option<String>,
);

impl Handoff {
    /// Handoffs of a task, oldest first.
    ///
    /// # Errors
    /// Fails when the query fails or a stored value is not understood.
    pub fn for_task(
        connection: &Connection,
        task_id: &str,
    ) -> Result<Vec<Self>, OrchestrationError> {
        let mut statement = connection.prepare(
            "SELECT id, task_id, commit_sha, base_sha, summary, changed_paths, reported_tests,
                    source, created_at_ms, reviewed_at_ms, review
             FROM task_handoffs WHERE task_id = ?1 ORDER BY created_at_ms, rowid",
        )?;
        let rows: Vec<HandoffRow> = statement
            .query_map([task_id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                ))
            })?
            .collect::<Result<_, _>>()?;
        let parse = |text: &str| {
            serde_json::from_str::<Vec<String>>(text)
                .map_err(|e| OrchestrationError::Corrupt(e.to_string()))
        };
        rows.into_iter()
            .map(|r| {
                Ok(Self {
                    id: r.0,
                    task_id: r.1,
                    commit_sha: r.2,
                    base_sha: r.3,
                    summary: r.4,
                    changed_paths: parse(&r.5)?,
                    reported_tests: parse(&r.6)?,
                    source: HandoffSource::parse(&r.7)?,
                    created_at_ms: r.8,
                    reviewed_at_ms: r.9,
                    review: r.10.as_deref().map(HandoffReview::parse).transpose()?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::open_in_memory;

    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn db_with_tasks(children: &[&str]) -> Connection {
        let connection = open_in_memory().unwrap();
        connection
            .execute_batch(
                "INSERT INTO repos (id, path, default_branch) VALUES ('r', '/x', 'main');
                 INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('lead', 'r', 'L', 'p', 'b', '/w', 'new', 0);",
            )
            .unwrap();
        for (i, id) in children.iter().enumerate() {
            connection
                .execute(
                    "INSERT INTO tasks (id, repo_id, parent_task_id, title, prompt, branch,
                                        worktree_path, state, created_at_ms)
                     VALUES (?1, 'r', 'lead', 'T', 'p', ?1, '/w', 'new', ?2)",
                    params![id, i64::try_from(i + 1).unwrap()],
                )
                .unwrap();
        }
        connection
    }

    fn handoff<'a>(id: &'a str, task: &'a str, sha: &'a str) -> NewHandoff<'a> {
        NewHandoff {
            id,
            task_id: task,
            commit_sha: sha,
            base_sha: SHA_B,
            summary: "done",
            changed_paths: &[],
            reported_tests: &[],
            source: HandoffSource::ReportedByAgent,
            created_at_ms: 1,
        }
    }

    fn state(connection: &Connection, id: &str) -> TaskState {
        load_graph(connection, "lead").unwrap().state(id).unwrap()
    }

    fn row_count(connection: &Connection, table: &str) -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    #[test]
    fn dependency_and_transition_survive_a_reload() {
        let mut db = db_with_tasks(&["a", "b"]);
        add_dependency(&mut db, "b", "a").unwrap();
        apply_transition(&mut db, "a", TaskState::Ready, 5).unwrap();
        let graph = load_graph(&db, "lead").unwrap();
        assert_eq!(graph.dependencies_of("b").unwrap(), vec!["a"]);
        assert_eq!(graph.state("a").unwrap(), TaskState::Ready);
        assert_eq!(graph.state("b").unwrap(), TaskState::Queued);
        let at: i64 = db
            .query_row(
                "SELECT updated_at_ms FROM task_orchestration WHERE task_id = 'a'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(at, 5);
    }

    #[test]
    fn a_cycle_is_refused_and_writes_nothing() {
        let mut db = db_with_tasks(&["a", "b", "c"]);
        add_dependency(&mut db, "b", "a").unwrap();
        add_dependency(&mut db, "c", "b").unwrap();
        let error = add_dependency(&mut db, "a", "c").unwrap_err();
        assert!(matches!(
            error,
            OrchestrationError::Graph(GraphError::Cycle(_))
        ));
        let unknown = add_dependency(&mut db, "a", "nope").unwrap_err();
        assert!(matches!(
            unknown,
            OrchestrationError::Graph(GraphError::UnknownTask(_))
        ));
        assert_eq!(row_count(&db, "task_dependencies"), 2);
    }

    #[test]
    fn a_failed_predecessor_blocks_dependents_transitively() {
        let mut db = db_with_tasks(&["a", "b", "c"]);
        add_dependency(&mut db, "b", "a").unwrap();
        add_dependency(&mut db, "c", "b").unwrap();
        apply_transition(&mut db, "a", TaskState::Ready, 1).unwrap();
        apply_transition(&mut db, "a", TaskState::Running, 2).unwrap();
        let changes = apply_transition(&mut db, "a", TaskState::Failed, 3).unwrap();
        assert_eq!(changes.len(), 3);
        assert_eq!(state(&db, "a"), TaskState::Failed);
        assert_eq!(state(&db, "b"), TaskState::Blocked);
        assert_eq!(state(&db, "c"), TaskState::Blocked);
    }

    #[test]
    fn completing_needs_an_approved_handoff() {
        let mut db = db_with_tasks(&["a"]);
        for (to, at) in [
            (TaskState::Ready, 1),
            (TaskState::Running, 2),
            (TaskState::ReviewRequired, 3),
        ] {
            apply_transition(&mut db, "a", to, at).unwrap();
        }
        let none = apply_transition(&mut db, "a", TaskState::Completed, 4).unwrap_err();
        assert!(matches!(
            none,
            OrchestrationError::HandoffNotApproved { .. }
        ));
        assert_eq!(state(&db, "a"), TaskState::ReviewRequired);

        record_handoff(&db, &handoff("h1", "a", SHA_A)).unwrap();
        let unreviewed = apply_transition(&mut db, "a", TaskState::Completed, 4).unwrap_err();
        assert!(matches!(
            unreviewed,
            OrchestrationError::HandoffNotApproved { .. }
        ));

        review_handoff(&db, "h1", HandoffReview::Approved, 5).unwrap();
        apply_transition(&mut db, "a", TaskState::Completed, 6).unwrap();
        assert_eq!(state(&db, "a"), TaskState::Completed);
    }

    #[test]
    fn a_newer_handoff_voids_an_earlier_approval() {
        let mut db = db_with_tasks(&["a"]);
        for to in [
            TaskState::Ready,
            TaskState::Running,
            TaskState::ReviewRequired,
        ] {
            apply_transition(&mut db, "a", to, 1).unwrap();
        }
        record_handoff(&db, &handoff("h1", "a", SHA_A)).unwrap();
        review_handoff(&db, "h1", HandoffReview::Approved, 2).unwrap();
        record_handoff(&db, &handoff("h2", "a", SHA_B)).unwrap();
        let error = apply_transition(&mut db, "a", TaskState::Completed, 3).unwrap_err();
        assert!(matches!(
            error,
            OrchestrationError::HandoffNotApproved { .. }
        ));
        assert!(matches!(
            review_handoff(&db, "h1", HandoffReview::Rejected, 4).unwrap_err(),
            OrchestrationError::AlreadyReviewed(_)
        ));
    }

    #[test]
    fn bad_commit_ids_are_refused() {
        let db = db_with_tasks(&["a"]);
        let upper = SHA_A.to_uppercase();
        let short = &SHA_A[..39];
        let non_hex = format!("{}g", &SHA_A[..39]);
        for bad in ["", "abc", upper.as_str(), short, non_hex.as_str()] {
            let error = record_handoff(&db, &handoff("h", "a", bad)).unwrap_err();
            assert!(
                matches!(error, OrchestrationError::InvalidCommitSha(_)),
                "{bad}"
            );
        }
        assert_eq!(row_count(&db, "task_handoffs"), 0);
        record_handoff(&db, &handoff("h64", "a", &"c".repeat(64))).unwrap();
        let stored = Handoff::for_task(&db, "a").unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].commit_sha.len(), 64);
    }

    #[test]
    fn deleting_a_task_removes_its_orchestration_rows() {
        let mut db = db_with_tasks(&["a", "b"]);
        add_dependency(&mut db, "b", "a").unwrap();
        apply_transition(&mut db, "b", TaskState::Cancelled, 1).unwrap();
        record_handoff(&db, &handoff("h1", "a", SHA_A)).unwrap();
        db.execute("DELETE FROM tasks WHERE id = 'a'", []).unwrap();
        assert_eq!(row_count(&db, "task_dependencies"), 0);
        assert_eq!(row_count(&db, "task_handoffs"), 0);
        assert_eq!(row_count(&db, "task_orchestration"), 1);
        db.execute("DELETE FROM tasks WHERE id = 'b'", []).unwrap();
        assert_eq!(row_count(&db, "task_orchestration"), 0);
    }
}
