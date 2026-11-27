//! Durable extra tabs of a task: agent and shell terminals, and the browser and file tabs that
//! point at an address or a file. Any number of each can be open at once.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};

use crate::StoreError;

/// What a tab runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabKind {
    /// A Claude Code session; the tab id is its conversation id.
    ClaudeCode,
    /// A Codex session.
    Codex,
    /// A plain shell.
    Shell,
    /// A web page; the tab's target is its address.
    Browser,
    /// A file of the task; the tab's target is its path inside the task's folder.
    File,
}

impl TabKind {
    /// The name stored in the database and sent to the window.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude_code",
            Self::Codex => "codex",
            Self::Shell => "shell",
            Self::Browser => "browser",
            Self::File => "file",
        }
    }

    /// Whether the tab runs an agent.
    #[must_use]
    pub const fn is_agent(self) -> bool {
        matches!(self, Self::ClaudeCode | Self::Codex)
    }

    /// Whether a terminal runs behind the tab.
    #[must_use]
    pub const fn has_terminal(self) -> bool {
        matches!(self, Self::ClaudeCode | Self::Codex | Self::Shell)
    }

    /// Parses a stored or requested kind name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "claude_code" => Some(Self::ClaudeCode),
            "codex" => Some(Self::Codex),
            "shell" => Some(Self::Shell),
            "browser" => Some(Self::Browser),
            "file" => Some(Self::File),
            _ => None,
        }
    }
}

/// One open extra tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskTab {
    /// Unique per tab; also names the terminal and, for Claude Code, its conversation.
    pub tab_id: String,
    /// What the tab runs.
    pub kind: TabKind,
    /// What the tab points at: the detected shell a shell tab runs (`None` is the default shell),
    /// a browser tab's address, or a file tab's path inside the task's folder.
    pub target: Option<String>,
}

fn read_tab(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<TaskTab>> {
    let kind: String = row.get(1)?;
    Ok(TabKind::parse(&kind).map(|kind| TaskTab {
        tab_id: row.get(0).unwrap_or_default(),
        kind,
        target: row.get(2).unwrap_or_default(),
    }))
}

/// Imports legacy second-agent IDs once, then lists every open tab by task in opening order.
///
/// # Errors
/// Fails when the database cannot be read or written.
pub fn load_task_tabs(
    connection: &mut Connection,
    legacy: &HashMap<String, String>,
) -> Result<HashMap<String, Vec<TaskTab>>, StoreError> {
    let transaction = connection.transaction()?;
    for (task_id, tab_id) in legacy {
        // The old single-tab table stays as the record of what was already imported or closed.
        let imported = transaction.execute(
            "INSERT INTO task_secondary_tabs (task_id, tab_id)
             SELECT id, ?2 FROM tasks WHERE id = ?1
             ON CONFLICT(task_id) DO NOTHING",
            params![task_id, tab_id],
        )?;
        if imported > 0 {
            transaction.execute(
                "INSERT OR IGNORE INTO task_tabs (task_id, tab_id, kind)
                 SELECT id, ?2, CASE WHEN agent_kind = 'codex' THEN 'claude_code' ELSE 'codex' END
                 FROM tasks WHERE id = ?1",
                params![task_id, tab_id],
            )?;
        }
    }
    let mut tabs: HashMap<String, Vec<TaskTab>> = HashMap::new();
    {
        let mut statement = transaction
            .prepare("SELECT tab_id, kind, target, task_id FROM task_tabs ORDER BY sort_order IS NULL, sort_order, rowid")?;
        let rows = statement.query_map([], |row| Ok((read_tab(row)?, row.get::<_, String>(3)?)))?;
        for row in rows {
            let (tab, task_id) = row?;
            if let Some(tab) = tab {
                tabs.entry(task_id).or_default().push(tab);
            }
        }
    }
    transaction.commit()?;
    Ok(tabs)
}

/// The open tabs of one task in opening order.
///
/// # Errors
/// Fails when the database cannot be read.
pub fn task_tabs(connection: &Connection, task_id: &str) -> Result<Vec<TaskTab>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT tab_id, kind, target FROM task_tabs WHERE task_id = ?1
         ORDER BY sort_order IS NULL, sort_order, rowid",
    )?;
    let rows = statement.query_map([task_id], read_tab)?;
    let mut tabs = Vec::new();
    for row in rows {
        tabs.extend(row?);
    }
    Ok(tabs)
}

/// Longest name a tab can be given.
pub const MAX_TAB_NAME_CHARS: usize = 60;

/// Names a tab, or clears the name with `None` or blank text so the automatic label returns.
///
/// # Errors
/// Fails with `NotFound` when the tab is not open, or `Invalid` when the name is too long.
pub fn rename_task_tab(
    connection: &Connection,
    task_id: &str,
    tab_id: &str,
    name: Option<&str>,
) -> Result<(), StoreError> {
    let name = name.map(str::trim).filter(|text| !text.is_empty());
    if name.is_some_and(|text| text.chars().count() > MAX_TAB_NAME_CHARS) {
        return Err(StoreError::Invalid(format!(
            "A tab name can have at most {MAX_TAB_NAME_CHARS} characters."
        )));
    }
    let changed = connection.execute(
        "UPDATE task_tabs SET name = ?3 WHERE task_id = ?1 AND tab_id = ?2",
        params![task_id, tab_id, name],
    )?;
    if changed == 0 {
        return Err(StoreError::NotFound {
            table: "task_tabs",
            id: tab_id.to_owned(),
        });
    }
    Ok(())
}

/// The names users gave tabs, by tab id.
///
/// # Errors
/// Fails when the database cannot be read.
pub fn task_tab_names(connection: &Connection) -> Result<HashMap<String, String>, StoreError> {
    let mut statement =
        connection.prepare("SELECT tab_id, name FROM task_tabs WHERE name IS NOT NULL")?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Places a task's open tabs in the given order. Tabs left out follow them in their earlier order.
///
/// # Errors
/// Fails with `NotFound` when a tab is not open on the task, or `Invalid` when a tab is listed
/// twice, and changes nothing.
pub fn reorder_task_tabs(
    connection: &mut Connection,
    task_id: &str,
    tab_ids: &[String],
) -> Result<(), StoreError> {
    let transaction = connection.transaction()?;
    // Every row, including kinds this build cannot show, so none is left with a stale position.
    let current: Vec<String> = {
        let mut statement = transaction.prepare(
            "SELECT tab_id FROM task_tabs WHERE task_id = ?1
             ORDER BY sort_order IS NULL, sort_order, rowid",
        )?;
        let rows = statement.query_map([task_id], |row| row.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut listed = HashSet::new();
    for tab_id in tab_ids {
        if !current.contains(tab_id) {
            return Err(StoreError::NotFound {
                table: "task_tabs",
                id: tab_id.clone(),
            });
        }
        if !listed.insert(tab_id.as_str()) {
            return Err(StoreError::Invalid(format!(
                "The tab {tab_id} is listed more than once."
            )));
        }
    }
    let left_out = current
        .iter()
        .filter(|tab_id| !listed.contains(tab_id.as_str()));
    for (position, tab_id) in tab_ids.iter().chain(left_out).enumerate() {
        transaction.execute(
            "UPDATE task_tabs SET sort_order = ?3 WHERE task_id = ?1 AND tab_id = ?2",
            params![task_id, tab_id, i64::try_from(position).unwrap_or(i64::MAX)],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

/// One open tab of a task, if it is still open.
///
/// # Errors
/// Fails when the database cannot be read.
pub fn task_tab(
    connection: &Connection,
    task_id: &str,
    tab_id: &str,
) -> Result<Option<TaskTab>, StoreError> {
    let tab = connection
        .query_row(
            "SELECT tab_id, kind, target FROM task_tabs WHERE task_id = ?1 AND tab_id = ?2",
            params![task_id, tab_id],
            read_tab,
        )
        .optional()?;
    Ok(tab.flatten())
}

/// The agent conversation recorded for a tab, such as the Codex conversation it started.
///
/// # Errors
/// Fails when the database cannot be read.
pub fn task_tab_conversation(
    connection: &Connection,
    task_id: &str,
    tab_id: &str,
) -> Result<Option<String>, StoreError> {
    let found: Option<Option<String>> = connection
        .query_row(
            "SELECT conversation_id FROM task_tabs WHERE task_id = ?1 AND tab_id = ?2",
            params![task_id, tab_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.flatten().filter(|id| !id.is_empty()))
}

/// Records the conversation a tab started, unless one is already recorded. Returns whether it
/// was recorded.
///
/// # Errors
/// Fails when the database cannot be written.
pub fn remember_task_tab_conversation(
    connection: &Connection,
    task_id: &str,
    tab_id: &str,
    conversation_id: &str,
) -> Result<bool, StoreError> {
    let changed = connection.execute(
        "UPDATE task_tabs SET conversation_id = ?3
         WHERE task_id = ?1 AND tab_id = ?2 AND (conversation_id IS NULL OR conversation_id = '')",
        params![task_id, tab_id, conversation_id],
    )?;
    Ok(changed > 0)
}

/// Opens one more tab. The same agent or shell can be added again; every tab has its own id.
///
/// # Errors
/// Fails when the task is not an open task, the id is taken, or the database cannot be written.
pub fn add_task_tab(
    connection: &Connection,
    task_id: &str,
    tab: &TaskTab,
) -> Result<(), StoreError> {
    let affected = connection.execute(
        "INSERT INTO task_tabs (task_id, tab_id, kind, target)
         SELECT id, ?2, ?3, ?4 FROM tasks WHERE id = ?1 AND archived_at_ms IS NULL",
        params![task_id, tab.tab_id, tab.kind.as_str(), tab.target],
    )?;
    if affected == 0 {
        return Err(StoreError::NotFound {
            table: "open tasks",
            id: task_id.to_owned(),
        });
    }
    Ok(())
}

/// Closes a tab and keeps its id until its program is proven stopped.
///
/// # Errors
/// Fails when the database cannot be written.
pub fn close_task_tab(
    connection: &Connection,
    task_id: &str,
    tab_id: &str,
) -> Result<(), StoreError> {
    // The program may outlive this row, so its id is kept until proven stopped.
    connection.execute(
        "INSERT OR IGNORE INTO task_retired_secondary_tabs (task_id, tab_id)
         SELECT task_id, tab_id FROM task_tabs WHERE task_id = ?1 AND tab_id = ?2",
        params![task_id, tab_id],
    )?;
    connection.execute(
        "DELETE FROM task_tabs WHERE task_id = ?1 AND tab_id = ?2",
        params![task_id, tab_id],
    )?;
    Ok(())
}

/// Tab IDs this task once had whose programs were not yet confirmed stopped.
///
/// # Errors
/// Fails when the database cannot be read.
pub fn retired_secondary_tabs(
    connection: &Connection,
    task_id: &str,
) -> Result<Vec<String>, StoreError> {
    let mut statement =
        connection.prepare("SELECT tab_id FROM task_retired_secondary_tabs WHERE task_id = ?1")?;
    let rows = statement.query_map([task_id], |row| row.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Drops a retired tab ID once its program is known to be stopped.
///
/// # Errors
/// Fails when the database cannot be written.
pub fn forget_retired_secondary_tab(
    connection: &Connection,
    task_id: &str,
    tab_id: &str,
) -> Result<(), StoreError> {
    connection.execute(
        "DELETE FROM task_retired_secondary_tabs WHERE task_id = ?1 AND tab_id = ?2",
        params![task_id, tab_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;

    use super::{
        add_task_tab, close_task_tab, forget_retired_secondary_tab, load_task_tabs,
        remember_task_tab_conversation, rename_task_tab, reorder_task_tabs, retired_secondary_tabs,
        task_tab, task_tab_conversation, task_tab_names, task_tabs, TabKind, TaskTab,
    };
    use crate::database::open_in_memory;
    use crate::{NewRepo, NewTask, RepoRecord, TaskRecord};

    fn connection_with_task() -> rusqlite::Connection {
        let connection = open_in_memory().expect("open");
        let repo = NewRepo {
            id: "r1",
            name: "brachio",
            path: Path::new("/src/brachio"),
            default_branch: "main",
            added_at_ms: 0,
            is_git: true,
        };
        RepoRecord::insert(&connection, &repo).expect("repo");
        let task = NewTask {
            id: "t1",
            repo_id: "r1",
            parent_task_id: None,
            title: "t",
            prompt: "p",
            branch: "b",
            worktree_path: Path::new("/w"),
            agent_kind: None,
            agent_session_id: None,
            compare_group: None,
            created_at_ms: 0,
        };
        TaskRecord::insert(&connection, &task).expect("task");
        connection
    }

    fn tab(id: &str, kind: TabKind) -> TaskTab {
        TaskTab {
            tab_id: id.to_owned(),
            kind,
            target: None,
        }
    }

    #[test]
    fn the_same_agent_can_be_open_twice_and_each_close_is_remembered() {
        let connection = connection_with_task();
        add_task_tab(&connection, "t1", &tab("a", TabKind::Codex)).expect("first");
        add_task_tab(&connection, "t1", &tab("b", TabKind::Codex)).expect("second");
        add_task_tab(&connection, "t1", &tab("c", TabKind::Shell)).expect("shell");
        let open: Vec<_> = task_tabs(&connection, "t1")
            .expect("list")
            .into_iter()
            .map(|tab| tab.tab_id)
            .collect();
        assert_eq!(open, ["a", "b", "c"]);
        assert!(add_task_tab(&connection, "t1", &tab("a", TabKind::Codex)).is_err());
        close_task_tab(&connection, "t1", "a").expect("close");
        assert!(task_tab(&connection, "t1", "a").expect("read").is_none());
        assert!(task_tab(&connection, "t1", "b").expect("read").is_some());
        assert_eq!(
            retired_secondary_tabs(&connection, "t1").expect("list"),
            ["a"]
        );
        forget_retired_secondary_tab(&connection, "t1", "a").expect("forget");
        assert!(retired_secondary_tabs(&connection, "t1")
            .expect("list")
            .is_empty());
    }

    fn open_ids(connection: &rusqlite::Connection) -> Vec<String> {
        task_tabs(connection, "t1")
            .expect("list")
            .into_iter()
            .map(|tab| tab.tab_id)
            .collect()
    }

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|&name| name.to_owned()).collect()
    }

    fn connection_with_ordered_tabs() -> rusqlite::Connection {
        let mut connection = connection_with_task();
        for id in ["a", "b", "c"] {
            add_task_tab(&connection, "t1", &tab(id, TabKind::Shell)).expect("tab");
        }
        reorder_task_tabs(&mut connection, "t1", &ids(&["a", "b", "c"])).expect("order");
        connection
    }

    #[test]
    fn tabs_left_out_of_a_reorder_follow_in_their_earlier_order() {
        let mut connection = connection_with_ordered_tabs();
        reorder_task_tabs(&mut connection, "t1", &ids(&["c"])).expect("partial");
        assert_eq!(open_ids(&connection), ["c", "a", "b"]);
        reorder_task_tabs(&mut connection, "t1", &ids(&["b", "c"])).expect("partial");
        assert_eq!(open_ids(&connection), ["b", "c", "a"]);
    }

    #[test]
    fn a_reorder_with_a_repeated_or_unknown_tab_changes_nothing() {
        let mut connection = connection_with_ordered_tabs();
        assert!(reorder_task_tabs(&mut connection, "t1", &ids(&["c", "a", "c"])).is_err());
        assert_eq!(open_ids(&connection), ["a", "b", "c"]);
        assert!(reorder_task_tabs(&mut connection, "t1", &ids(&["c", "gone"])).is_err());
        assert_eq!(open_ids(&connection), ["a", "b", "c"]);
    }

    #[test]
    fn a_tab_keeps_the_first_conversation_recorded_for_it() {
        let connection = connection_with_task();
        add_task_tab(&connection, "t1", &tab("a", TabKind::Codex)).expect("tab");
        add_task_tab(&connection, "t1", &tab("b", TabKind::Codex)).expect("tab");
        assert_eq!(
            task_tab_conversation(&connection, "t1", "a").expect("read"),
            None
        );
        assert!(remember_task_tab_conversation(&connection, "t1", "a", "first").expect("write"));
        assert!(!remember_task_tab_conversation(&connection, "t1", "a", "second").expect("write"));
        assert!(!remember_task_tab_conversation(&connection, "t1", "gone", "x").expect("write"));
        assert_eq!(
            task_tab_conversation(&connection, "t1", "a")
                .expect("read")
                .as_deref(),
            Some("first")
        );
        assert_eq!(
            task_tab_conversation(&connection, "t1", "b").expect("read"),
            None
        );
        close_task_tab(&connection, "t1", "a").expect("close");
        assert_eq!(
            task_tab_conversation(&connection, "t1", "a").expect("read"),
            None
        );
    }

    #[test]
    fn a_tab_name_is_kept_trimmed_and_cleared_by_blank_text() {
        let connection = connection_with_task();
        add_task_tab(&connection, "t1", &tab("a", TabKind::Shell)).expect("tab");
        rename_task_tab(&connection, "t1", "a", Some("  build  ")).expect("rename");
        assert_eq!(task_tab_names(&connection).expect("names")["a"], "build");
        assert!(rename_task_tab(&connection, "t1", "a", Some(&"x".repeat(61))).is_err());
        assert!(rename_task_tab(&connection, "t1", "gone", Some("x")).is_err());
        rename_task_tab(&connection, "t1", "a", Some(" ")).expect("clear");
        assert!(task_tab_names(&connection).expect("names").is_empty());
    }

    #[test]
    fn a_legacy_second_agent_is_imported_once() {
        let mut connection = connection_with_task();
        let legacy = HashMap::from([("t1".to_owned(), "old".to_owned())]);
        let tabs = load_task_tabs(&mut connection, &legacy).expect("import");
        assert_eq!(tabs["t1"], [tab("old", TabKind::Codex)]);
        close_task_tab(&connection, "t1", "old").expect("close");
        assert!(load_task_tabs(&mut connection, &legacy)
            .expect("reload")
            .is_empty());
    }

    #[test]
    fn browser_and_file_tabs_keep_their_target_and_unknown_kinds_are_skipped() {
        let connection = connection_with_task();
        let with_target = |id: &str, kind, target: &str| TaskTab {
            tab_id: id.to_owned(),
            kind,
            target: Some(target.to_owned()),
        };
        add_task_tab(
            &connection,
            "t1",
            &with_target("w", TabKind::Browser, "https://example.com/"),
        )
        .expect("browser");
        add_task_tab(
            &connection,
            "t1",
            &with_target("f", TabKind::File, "docs/README.md"),
        )
        .expect("file");
        connection
            .execute(
                "INSERT INTO task_tabs (task_id, tab_id, kind) VALUES ('t1', 'future', 'hologram')",
                [],
            )
            .expect("a kind from a newer build");

        let tabs = task_tabs(&connection, "t1").expect("list");
        assert_eq!(
            tabs,
            [
                with_target("w", TabKind::Browser, "https://example.com/"),
                with_target("f", TabKind::File, "docs/README.md"),
            ]
        );
        assert!(task_tab(&connection, "t1", "future")
            .expect("read")
            .is_none());
    }

    #[test]
    fn only_agents_and_shells_have_a_terminal() {
        assert!(TabKind::Codex.is_agent() && TabKind::ClaudeCode.is_agent());
        assert!(!TabKind::Shell.is_agent() && !TabKind::Browser.is_agent());
        assert!(TabKind::Shell.has_terminal());
        assert!(!TabKind::Browser.has_terminal() && !TabKind::File.has_terminal());
        assert_eq!(TabKind::parse("browser"), Some(TabKind::Browser));
        assert_eq!(TabKind::parse("file"), Some(TabKind::File));
        assert_eq!(TabKind::parse("hologram"), None);
    }
}
