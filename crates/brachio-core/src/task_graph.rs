//! Dependency-aware task state machine (docs/agent-orchestration-design.md).
//!
//! Pure model: no storage, clock or scheduling. Callers decide when to start tasks.
//!
//! Rules beyond the design doc, which is silent on them:
//! - `review_required -> running` is rework after a rejected review.
//! - `failed` and `cancelled` may be explicitly re-queued (retry/reassignment);
//!   dependents stay `blocked` until someone explicitly re-queues them, and that
//!   is refused while any dependency is still failed, cancelled or blocked.
//! - `blocked` is only entered as a consequence of a dependency failing or being
//!   cancelled; it can never be requested directly.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Lifecycle state of a task; the strings are stable for storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Waiting for dependencies; not yet cleared to start.
    Queued,
    /// Every dependency is completed; may be started by a caller.
    Ready,
    /// An agent is working on it.
    Running,
    /// The agent finished; a human must approve before dependents proceed.
    ReviewRequired,
    /// Reviewed and approved. Terminal.
    Completed,
    /// A dependency failed or was cancelled.
    Blocked,
    /// Failed, or rejected in review.
    Failed,
    /// Cancelled by the user.
    Cancelled,
}

impl TaskState {
    /// Every state, in declaration order.
    pub const ALL: [TaskState; 8] = [
        TaskState::Queued,
        TaskState::Ready,
        TaskState::Running,
        TaskState::ReviewRequired,
        TaskState::Completed,
        TaskState::Blocked,
        TaskState::Failed,
        TaskState::Cancelled,
    ];

    /// The stable snake_case name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            TaskState::Queued => "queued",
            TaskState::Ready => "ready",
            TaskState::Running => "running",
            TaskState::ReviewRequired => "review_required",
            TaskState::Completed => "completed",
            TaskState::Blocked => "blocked",
            TaskState::Failed => "failed",
            TaskState::Cancelled => "cancelled",
        }
    }

    /// Whether the table permits `self -> to`, before dependency guards.
    #[must_use]
    pub fn can_transition_to(self, to: TaskState) -> bool {
        use TaskState::{
            Blocked, Cancelled, Completed, Failed, Queued, Ready, ReviewRequired, Running,
        };
        matches!(
            (self, to),
            (Queued, Ready)
                | (Queued, Cancelled)
                | (Ready, Running)
                | (Ready, Cancelled)
                | (Running, ReviewRequired)
                | (Running, Failed)
                | (Running, Cancelled)
                | (ReviewRequired, Completed)
                | (ReviewRequired, Failed)
                | (ReviewRequired, Running)
                | (ReviewRequired, Cancelled)
                | (Blocked, Queued)
                | (Blocked, Cancelled)
                | (Failed, Queued)
                | (Cancelled, Queued)
        )
    }
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A string that is not a [`TaskState`] name.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown task state: {0}")]
pub struct ParseTaskStateError(pub String);

impl FromStr for TaskState {
    type Err = ParseTaskStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TaskState::ALL
            .into_iter()
            .find(|state| state.as_str() == s)
            .ok_or_else(|| ParseTaskStateError(s.to_owned()))
    }
}

/// Why a graph operation was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GraphError {
    /// The id was never added.
    #[error("unknown task: {0}")]
    UnknownTask(String),
    /// The id was already added.
    #[error("task already exists: {0}")]
    DuplicateTask(String),
    /// A task listed itself as a dependency.
    #[error("task cannot depend on itself: {0}")]
    SelfDependency(String),
    /// The edge is already present.
    #[error("dependency already exists: {task} -> {depends_on}")]
    DuplicateDependency {
        /// The dependent task.
        task: String,
        /// The task it already depends on.
        depends_on: String,
    },
    /// The edge would close a loop; the path starts and ends at the same task.
    #[error("dependency would create a cycle: {}", .0.join(" -> "))]
    Cycle(Vec<String>),
    /// Dependencies of a task that already started cannot change.
    #[error("dependencies of {id} can only change while it is queued or blocked, not {state}")]
    DependenciesLocked {
        /// The task whose dependencies were edited.
        id: String,
        /// Its current state.
        state: TaskState,
    },
    /// The state table or a dependency guard forbids the move.
    #[error("invalid transition for {id}: {from} -> {to}")]
    InvalidTransition {
        /// The task being moved.
        id: String,
        /// Its current state.
        from: TaskState,
        /// The requested state.
        to: TaskState,
    },
}

/// One task state change produced by a transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The task that changed.
    pub id: String,
    /// State before the change.
    pub from: TaskState,
    /// State after the change.
    pub to: TaskState,
}

#[derive(Debug)]
struct Node {
    id: String,
    state: TaskState,
    dependencies: Vec<usize>,
    dependents: Vec<usize>,
}

/// Tasks and their dependency edges, with deterministic insertion-order iteration.
#[derive(Debug, Default)]
pub struct TaskGraph {
    nodes: Vec<Node>,
    index: HashMap<String, usize>,
}

impl TaskGraph {
    /// An empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a queued task.
    ///
    /// # Errors
    /// [`GraphError::DuplicateTask`] if the id exists.
    pub fn add_task(&mut self, id: &str) -> Result<(), GraphError> {
        if self.index.contains_key(id) {
            return Err(GraphError::DuplicateTask(id.to_owned()));
        }
        self.index.insert(id.to_owned(), self.nodes.len());
        self.nodes.push(Node {
            id: id.to_owned(),
            state: TaskState::Queued,
            dependencies: Vec::new(),
            dependents: Vec::new(),
        });
        Ok(())
    }

    /// Current state of a task.
    ///
    /// # Errors
    /// [`GraphError::UnknownTask`] if the id was never added.
    pub fn state(&self, id: &str) -> Result<TaskState, GraphError> {
        Ok(self.nodes[self.lookup(id)?].state)
    }

    /// Makes `task` depend on `depends_on`.
    ///
    /// # Errors
    /// Unknown ids, self or duplicate edges, a cycle, or a task that already
    /// started (only queued or blocked tasks can gain dependencies).
    pub fn add_dependency(&mut self, task: &str, depends_on: &str) -> Result<(), GraphError> {
        let task_idx = self.lookup(task)?;
        let dep_idx = self.lookup(depends_on)?;
        if task_idx == dep_idx {
            return Err(GraphError::SelfDependency(task.to_owned()));
        }
        if self.nodes[task_idx].dependencies.contains(&dep_idx) {
            return Err(GraphError::DuplicateDependency {
                task: task.to_owned(),
                depends_on: depends_on.to_owned(),
            });
        }
        let state = self.nodes[task_idx].state;
        if !matches!(state, TaskState::Queued | TaskState::Blocked) {
            return Err(GraphError::DependenciesLocked {
                id: task.to_owned(),
                state,
            });
        }
        if let Some(path) = self.dependency_path(dep_idx, task_idx) {
            let mut cycle = vec![task.to_owned()];
            cycle.extend(path.into_iter().map(|i| self.nodes[i].id.clone()));
            return Err(GraphError::Cycle(cycle));
        }
        self.nodes[task_idx].dependencies.push(dep_idx);
        self.nodes[dep_idx].dependents.push(task_idx);
        Ok(())
    }

    /// Direct dependencies of a task, in the order they were added.
    ///
    /// # Errors
    /// [`GraphError::UnknownTask`] if the id was never added.
    pub fn dependencies_of(&self, id: &str) -> Result<Vec<&str>, GraphError> {
        let idx = self.lookup(id)?;
        Ok(self.ids(&self.nodes[idx].dependencies))
    }

    /// Tasks that directly depend on this one, in the order they were added.
    ///
    /// # Errors
    /// [`GraphError::UnknownTask`] if the id was never added.
    pub fn dependents_of(&self, id: &str) -> Result<Vec<&str>, GraphError> {
        let idx = self.lookup(id)?;
        Ok(self.ids(&self.nodes[idx].dependents))
    }

    /// Queued tasks whose dependencies are all completed, in insertion order.
    #[must_use]
    pub fn ready_tasks(&self) -> Vec<&str> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(idx, node)| node.state == TaskState::Queued && self.deps_completed(*idx))
            .map(|(_, node)| node.id.as_str())
            .collect()
    }

    /// Applies `id -> to`. Failure or cancellation also blocks queued dependents
    /// transitively; the returned list is the primary change, then those
    /// consequences in breadth-first order.
    ///
    /// # Errors
    /// [`GraphError::UnknownTask`], or [`GraphError::InvalidTransition`] when the
    /// state table or a dependency guard forbids the move.
    pub fn transition(&mut self, id: &str, to: TaskState) -> Result<Vec<Change>, GraphError> {
        let idx = self.lookup(id)?;
        let from = self.nodes[idx].state;
        let invalid = || GraphError::InvalidTransition {
            id: id.to_owned(),
            from,
            to,
        };
        if !from.can_transition_to(to) {
            return Err(invalid());
        }
        let guard_ok = match to {
            TaskState::Ready => self.deps_completed(idx),
            TaskState::Queued if from == TaskState::Blocked => {
                self.nodes[idx].dependencies.iter().all(|&d| {
                    !matches!(
                        self.nodes[d].state,
                        TaskState::Failed | TaskState::Cancelled | TaskState::Blocked
                    )
                })
            }
            _ => true,
        };
        if !guard_ok {
            return Err(invalid());
        }

        self.nodes[idx].state = to;
        let mut changes = vec![Change {
            id: id.to_owned(),
            from,
            to,
        }];
        if matches!(to, TaskState::Failed | TaskState::Cancelled) {
            self.block_dependents(idx, &mut changes);
        }
        Ok(changes)
    }

    fn block_dependents(&mut self, origin: usize, changes: &mut Vec<Change>) {
        let mut pending = VecDeque::from([origin]);
        while let Some(current) = pending.pop_front() {
            for dependent in self.nodes[current].dependents.clone() {
                let from = self.nodes[dependent].state;
                if from != TaskState::Queued {
                    continue;
                }
                self.nodes[dependent].state = TaskState::Blocked;
                changes.push(Change {
                    id: self.nodes[dependent].id.clone(),
                    from,
                    to: TaskState::Blocked,
                });
                pending.push_back(dependent);
            }
        }
    }

    fn deps_completed(&self, idx: usize) -> bool {
        self.nodes[idx]
            .dependencies
            .iter()
            .all(|&d| self.nodes[d].state == TaskState::Completed)
    }

    fn lookup(&self, id: &str) -> Result<usize, GraphError> {
        self.index
            .get(id)
            .copied()
            .ok_or_else(|| GraphError::UnknownTask(id.to_owned()))
    }

    fn ids(&self, indices: &[usize]) -> Vec<&str> {
        indices.iter().map(|&i| self.nodes[i].id.as_str()).collect()
    }

    /// Path from `from` to `target` following dependency edges, inclusive.
    fn dependency_path(&self, from: usize, target: usize) -> Option<Vec<usize>> {
        let mut visited = vec![false; self.nodes.len()];
        let mut path = Vec::new();
        self.walk(from, target, &mut visited, &mut path)
            .then_some(path)
    }

    fn walk(
        &self,
        current: usize,
        target: usize,
        visited: &mut [bool],
        path: &mut Vec<usize>,
    ) -> bool {
        path.push(current);
        if current == target {
            return true;
        }
        if !std::mem::replace(&mut visited[current], true) {
            for &next in &self.nodes[current].dependencies {
                if self.walk(next, target, visited, path) {
                    return true;
                }
            }
        }
        path.pop();
        false
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::TaskState::{
        Blocked, Cancelled, Completed, Failed, Queued, Ready, ReviewRequired, Running,
    };
    use super::*;

    fn graph(ids: &[&str], edges: &[(&str, &str)]) -> TaskGraph {
        let mut g = TaskGraph::new();
        for id in ids {
            g.add_task(id).unwrap();
        }
        for (task, dep) in edges {
            g.add_dependency(task, dep).unwrap();
        }
        g
    }

    fn diamond() -> TaskGraph {
        graph(
            &["A", "B", "C", "D"],
            &[("B", "A"), ("C", "A"), ("D", "B"), ("D", "C")],
        )
    }

    fn change(id: &str, from: TaskState, to: TaskState) -> Change {
        Change {
            id: id.to_owned(),
            from,
            to,
        }
    }

    #[test]
    fn rejects_self_dependency() {
        let mut g = graph(&["A"], &[]);
        assert_eq!(
            g.add_dependency("A", "A"),
            Err(GraphError::SelfDependency("A".into()))
        );
    }

    #[test]
    fn rejects_direct_cycle() {
        let mut g = graph(&["A", "B"], &[("B", "A")]);
        assert_eq!(
            g.add_dependency("A", "B"),
            Err(GraphError::Cycle(vec!["A".into(), "B".into(), "A".into()]))
        );
    }

    #[test]
    fn rejects_indirect_cycle_and_names_path() {
        let mut g = graph(&["A", "B", "C"], &[("B", "A"), ("C", "B")]);
        assert_eq!(
            g.add_dependency("A", "C"),
            Err(GraphError::Cycle(vec![
                "A".into(),
                "C".into(),
                "B".into(),
                "A".into()
            ]))
        );
        assert!(g.dependencies_of("A").unwrap().is_empty());
    }

    #[test]
    fn diamond_is_not_a_cycle() {
        let g = diamond();
        assert_eq!(g.dependencies_of("D").unwrap(), vec!["B", "C"]);
        assert_eq!(g.dependents_of("A").unwrap(), vec!["B", "C"]);
    }

    #[test]
    fn rejects_unknown_ids() {
        let mut g = graph(&["A"], &[]);
        assert_eq!(
            g.add_dependency("A", "X"),
            Err(GraphError::UnknownTask("X".into()))
        );
        assert_eq!(
            g.add_dependency("X", "A"),
            Err(GraphError::UnknownTask("X".into()))
        );
        assert_eq!(
            g.transition("X", Ready),
            Err(GraphError::UnknownTask("X".into()))
        );
        assert_eq!(
            g.dependents_of("X"),
            Err(GraphError::UnknownTask("X".into()))
        );
    }

    #[test]
    fn rejects_duplicate_edge_and_duplicate_task() {
        let mut g = graph(&["A", "B"], &[("B", "A")]);
        assert_eq!(
            g.add_dependency("B", "A"),
            Err(GraphError::DuplicateDependency {
                task: "B".into(),
                depends_on: "A".into()
            })
        );
        assert_eq!(g.add_task("A"), Err(GraphError::DuplicateTask("A".into())));
    }

    #[test]
    fn dependencies_cannot_change_once_ready() {
        let mut g = graph(&["A", "B"], &[]);
        g.transition("B", Ready).unwrap();
        assert_eq!(
            g.add_dependency("B", "A"),
            Err(GraphError::DependenciesLocked {
                id: "B".into(),
                state: Ready
            })
        );
    }

    #[test]
    fn allowed_transition_set_is_exact() {
        let allowed = [
            (Queued, Ready),
            (Queued, Cancelled),
            (Ready, Running),
            (Ready, Cancelled),
            (Running, ReviewRequired),
            (Running, Failed),
            (Running, Cancelled),
            (ReviewRequired, Completed),
            (ReviewRequired, Failed),
            (ReviewRequired, Running),
            (ReviewRequired, Cancelled),
            (Blocked, Queued),
            (Blocked, Cancelled),
            (Failed, Queued),
            (Cancelled, Queued),
        ];
        for from in TaskState::ALL {
            for to in TaskState::ALL {
                assert_eq!(
                    from.can_transition_to(to),
                    allowed.contains(&(from, to)),
                    "{from} -> {to}"
                );
            }
        }
    }

    #[test]
    fn completed_is_terminal() {
        let mut g = graph(&["A"], &[]);
        for to in [Ready, Running, ReviewRequired, Completed] {
            g.transition("A", to).unwrap();
        }
        for to in TaskState::ALL {
            assert!(matches!(
                g.transition("A", to),
                Err(GraphError::InvalidTransition { .. })
            ));
        }
    }

    #[test]
    fn blocked_cannot_be_requested_directly() {
        let mut g = graph(&["A"], &[]);
        assert_eq!(
            g.transition("A", Blocked),
            Err(GraphError::InvalidTransition {
                id: "A".into(),
                from: Queued,
                to: Blocked
            })
        );
    }

    #[test]
    fn ready_refused_until_all_dependencies_completed() {
        let mut g = graph(&["A", "B", "C"], &[("C", "A"), ("C", "B")]);
        let err = GraphError::InvalidTransition {
            id: "C".into(),
            from: Queued,
            to: Ready,
        };
        assert_eq!(g.transition("C", Ready), Err(err.clone()));
        for to in [Ready, Running, ReviewRequired, Completed] {
            g.transition("A", to).unwrap();
        }
        assert_eq!(g.transition("C", Ready), Err(err));
        for to in [Ready, Running, ReviewRequired, Completed] {
            g.transition("B", to).unwrap();
        }
        assert_eq!(
            g.transition("C", Ready).unwrap(),
            vec![change("C", Queued, Ready)]
        );
    }

    #[test]
    fn review_required_predecessor_does_not_unblock_dependents() {
        let mut g = graph(&["A", "B"], &[("B", "A")]);
        for to in [Ready, Running, ReviewRequired] {
            g.transition("A", to).unwrap();
        }
        assert_eq!(g.ready_tasks(), Vec::<&str>::new());
        assert!(g.transition("B", Ready).is_err());
        g.transition("A", Completed).unwrap();
        assert_eq!(g.ready_tasks(), vec!["B"]);
    }

    #[test]
    fn failed_predecessor_blocks_diamond_in_order() {
        let mut g = diamond();
        for to in [Ready, Running] {
            g.transition("A", to).unwrap();
        }
        let changes = g.transition("A", Failed).unwrap();
        assert_eq!(
            changes,
            vec![
                change("A", Running, Failed),
                change("B", Queued, Blocked),
                change("C", Queued, Blocked),
                change("D", Queued, Blocked),
            ]
        );
        assert_eq!(g.ready_tasks(), Vec::<&str>::new());
    }

    #[test]
    fn cancelled_predecessor_blocks_dependents_transitively() {
        let mut g = diamond();
        let changes = g.transition("B", Cancelled).unwrap();
        assert_eq!(
            changes,
            vec![change("B", Queued, Cancelled), change("D", Queued, Blocked)]
        );
        assert_eq!(g.state("C").unwrap(), Queued);
    }

    #[test]
    fn rejected_review_blocks_dependents() {
        let mut g = graph(&["A", "B"], &[("B", "A")]);
        for to in [Ready, Running, ReviewRequired] {
            g.transition("A", to).unwrap();
        }
        let changes = g.transition("A", Failed).unwrap();
        assert_eq!(
            changes,
            vec![
                change("A", ReviewRequired, Failed),
                change("B", Queued, Blocked)
            ]
        );
    }

    #[test]
    fn retry_does_not_unblock_dependents_automatically() {
        let mut g = graph(&["A", "B"], &[("B", "A")]);
        g.transition("A", Cancelled).unwrap();
        assert_eq!(g.state("B").unwrap(), Blocked);

        g.transition("A", Queued).unwrap();
        assert_eq!(g.state("B").unwrap(), Blocked);
        assert_eq!(g.ready_tasks(), vec!["A"]);
        assert!(g.transition("B", Ready).is_err());
        assert_eq!(
            g.transition("B", Queued).unwrap(),
            vec![change("B", Blocked, Queued)]
        );
    }

    #[test]
    fn blocked_requeue_refused_while_dependency_failed() {
        let mut g = graph(&["A", "B"], &[("B", "A")]);
        for to in [Ready, Running, Failed] {
            g.transition("A", to).unwrap();
        }
        assert!(matches!(
            g.transition("B", Queued),
            Err(GraphError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn ready_tasks_in_insertion_order_without_mutation() {
        let mut g = graph(&["Z", "M", "A", "D"], &[("D", "Z")]);
        assert_eq!(g.ready_tasks(), vec!["Z", "M", "A"]);
        assert_eq!(g.ready_tasks(), vec!["Z", "M", "A"]);
        assert_eq!(g.state("Z").unwrap(), Queued);
        g.transition("M", Ready).unwrap();
        assert_eq!(g.ready_tasks(), vec!["Z", "A"]);
    }

    #[test]
    fn state_strings_round_trip() {
        let expected = [
            (Queued, "queued"),
            (Ready, "ready"),
            (Running, "running"),
            (ReviewRequired, "review_required"),
            (Completed, "completed"),
            (Blocked, "blocked"),
            (Failed, "failed"),
            (Cancelled, "cancelled"),
        ];
        for (state, text) in expected {
            assert_eq!(state.to_string(), text);
            assert_eq!(text.parse::<TaskState>(), Ok(state));
            let json = serde_json::to_string(&state).unwrap();
            assert_eq!(json, format!("\"{text}\""));
            assert_eq!(serde_json::from_str::<TaskState>(&json).unwrap(), state);
        }
        assert_eq!(
            "Queued".parse::<TaskState>(),
            Err(ParseTaskStateError("Queued".into()))
        );
    }
}
