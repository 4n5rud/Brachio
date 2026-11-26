//! IPC for a task's extra tabs: terminal identities that outlive the window, and the browser and
//! file tabs that point at an address or a file.

use std::collections::HashMap;

use brachio_store::{TabKind, TaskTab};
use serde::Serialize;
use tauri::State;

use crate::workspace::Database;

/// An open extra tab as the window sees it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabInfo {
    id: String,
    kind: &'static str,
    /// The shell of a shell tab; kept for the window's existing code.
    shell: Option<String>,
    /// What the tab points at: its shell, address or file.
    target: Option<String>,
    name: Option<String>,
}

impl From<TaskTab> for TabInfo {
    fn from(tab: TaskTab) -> Self {
        Self {
            id: tab.tab_id,
            kind: tab.kind.as_str(),
            shell: (tab.kind == TabKind::Shell)
                .then(|| tab.target.clone())
                .flatten(),
            target: tab.target,
            name: None,
        }
    }
}

#[tauri::command]
pub fn task_open_tabs(
    database: State<'_, Database>,
    mut legacy: HashMap<String, String>,
) -> Result<HashMap<String, Vec<TabInfo>>, String> {
    legacy.retain(|_, tab| uuid::Uuid::parse_str(tab).is_ok());
    let mut connection = database.lock()?;
    let tabs = brachio_store::load_task_tabs(&mut connection, &legacy)
        .map_err(|error| error.to_string())?;
    let names = brachio_store::task_tab_names(&connection).map_err(|error| error.to_string())?;
    Ok(tabs
        .into_iter()
        .map(|(task, tabs)| {
            let tabs = tabs
                .into_iter()
                .map(|tab| {
                    let name = names.get(&tab.tab_id).cloned();
                    TabInfo {
                        name,
                        ..TabInfo::from(tab)
                    }
                })
                .collect();
            (task, tabs)
        })
        .collect())
}

/// Checks what a new file or browser tab would point at and returns it in the form a stored tab
/// keeps, so the window can find a tab already showing it and refuse a bad one before anything
/// closes. The same rule as `task_add_tab`, kept in one place.
#[tauri::command]
pub fn task_tab_target(kind: String, target: String) -> Result<String, String> {
    let kind = TabKind::parse(&kind).ok_or_else(|| "Unknown tab kind.".to_owned())?;
    if !matches!(kind, TabKind::Browser | TabKind::File) {
        return Err("Only a file or browser tab has a target.".to_owned());
    }
    checked_target(kind, None, Some(target), |_| false)?
        .ok_or_else(|| "This tab needs a target.".to_owned())
}

/// Stores a new tab and returns its target as stored, which may be written differently from the
/// one asked for (a cleaned path or a trimmed address).
#[tauri::command]
pub fn task_add_tab(
    database: State<'_, Database>,
    task_id: String,
    tab_id: String,
    kind: String,
    shell: Option<String>,
    target: Option<String>,
) -> Result<Option<String>, String> {
    if uuid::Uuid::parse_str(&tab_id).is_err() {
        return Err("Invalid tab ID.".to_owned());
    }
    let kind = TabKind::parse(&kind).ok_or_else(|| "Unknown tab kind.".to_owned())?;
    let target = checked_target(kind, shell, target, |id| {
        crate::terminal_shells::find(id).is_some()
    })?;
    let connection = database.lock()?;
    brachio_store::add_task_tab(
        &connection,
        &task_id,
        &TaskTab {
            tab_id,
            kind,
            target: target.clone(),
        },
    )
    .map_err(|error| error.to_string())?;
    // A shell tab's target is its shell, which the window already knows as `shell`.
    Ok(target.filter(|_| matches!(kind, TabKind::Browser | TabKind::File)))
}

/// The longest address a browser tab keeps.
const MAX_ADDRESS_CHARS: usize = 2048;

/// A page address a browser tab may open: `http` or `https`, with a host, no spaces or control
/// characters, and no user name or password in front of the host.
fn web_address(text: &str) -> Result<String, String> {
    let address = text.trim();
    let invalid = || "A browser tab needs an http or https address.".to_owned();
    if address.is_empty()
        || address.chars().count() > MAX_ADDRESS_CHARS
        || address.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(invalid());
    }
    let lower = address.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .ok_or_else(invalid)?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if host.is_empty() || host.contains('@') {
        return Err(invalid());
    }
    Ok(address.to_owned())
}

/// What a new tab of `kind` may point at, checked per kind. `shell_known` tells whether a shell is installed.
fn checked_target(
    kind: TabKind,
    shell: Option<String>,
    target: Option<String>,
    shell_known: impl Fn(&str) -> bool,
) -> Result<Option<String>, String> {
    match kind {
        TabKind::ClaudeCode | TabKind::Codex => {
            if shell.is_some() || target.is_some() {
                return Err("An agent tab has no shell or target.".to_owned());
            }
            Ok(None)
        }
        TabKind::Shell => {
            if target.is_some() {
                return Err("A shell tab names its shell, not a target.".to_owned());
            }
            if shell.as_deref().is_some_and(|id| !shell_known(id)) {
                return Err("That shell is not installed.".to_owned());
            }
            Ok(shell)
        }
        TabKind::Browser => {
            if shell.is_some() {
                return Err("Only a shell tab can name a shell.".to_owned());
            }
            web_address(&target.ok_or("A browser tab needs an address.")?).map(Some)
        }
        TabKind::File => {
            if shell.is_some() {
                return Err("Only a shell tab can name a shell.".to_owned());
            }
            let path = target.ok_or("A file tab needs a path.")?;
            let clean = crate::task_file_ops::clean_relative(&path)?;
            let text = clean.to_string_lossy().replace('\\', "/");
            if text.is_empty() {
                return Err("A file tab needs a path inside the task.".to_owned());
            }
            Ok(Some(text))
        }
    }
}

#[tauri::command]
pub fn task_close_tab(
    database: State<'_, Database>,
    task_id: String,
    tab_id: String,
) -> Result<(), String> {
    let connection = database.lock()?;
    brachio_store::close_task_tab(&connection, &task_id, &tab_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_rename_tab(
    database: State<'_, Database>,
    task_id: String,
    tab_id: String,
    name: Option<String>,
) -> Result<(), String> {
    let connection = database.lock()?;
    brachio_store::rename_task_tab(&connection, &task_id, &tab_id, name.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn task_reorder_tabs(
    database: State<'_, Database>,
    task_id: String,
    tab_ids: Vec<String>,
) -> Result<(), String> {
    let mut connection = database.lock()?;
    brachio_store::reorder_task_tabs(&mut connection, &task_id, &tab_ids)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{checked_target, task_tab_target, web_address, TabKind};

    #[test]
    fn a_content_target_comes_back_as_it_is_stored() {
        assert_eq!(
            task_tab_target("file".into(), "./docs/./a.md".into()),
            Ok("docs/a.md".to_owned())
        );
        assert_eq!(
            task_tab_target("browser".into(), "  https://a.dev/ ".into()),
            Ok("https://a.dev/".to_owned())
        );
        assert!(task_tab_target("file".into(), "../x".into()).is_err());
        assert!(task_tab_target("shell".into(), "zsh".into()).is_err());
        assert!(task_tab_target("nope".into(), "a".into()).is_err());
    }

    fn known(id: &str) -> bool {
        id == "zsh"
    }

    #[test]
    fn a_browser_tab_takes_only_web_addresses() {
        assert!(web_address("https://example.com/a?b=1#c").is_ok());
        assert!(web_address("  HTTP://localhost:5173 ").is_ok());
        for bad in [
            "",
            "example.com",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://",
            "https:///path",
            "https://user:pass@example.com/",
            "https://exa mple.com/",
            "ftp://example.com/",
        ] {
            assert!(web_address(bad).is_err(), "{bad} must be refused");
        }
        assert!(web_address(&format!("https://example.com/{}", "a".repeat(2100))).is_err());
    }

    #[test]
    fn each_kind_accepts_only_its_own_target() {
        let some = |text: &str| Some(text.to_owned());
        assert_eq!(checked_target(TabKind::Codex, None, None, known), Ok(None));
        assert!(checked_target(TabKind::Codex, some("zsh"), None, known).is_err());
        assert_eq!(
            checked_target(TabKind::Shell, some("zsh"), None, known),
            Ok(some("zsh"))
        );
        assert_eq!(checked_target(TabKind::Shell, None, None, known), Ok(None));
        assert!(checked_target(TabKind::Shell, some("fish"), None, known).is_err());
        assert!(checked_target(TabKind::Shell, None, some("x"), known).is_err());
        assert_eq!(
            checked_target(TabKind::Browser, None, some("https://a.dev/"), known),
            Ok(some("https://a.dev/"))
        );
        assert!(checked_target(TabKind::Browser, None, None, known).is_err());
        assert!(
            checked_target(TabKind::Browser, some("zsh"), some("https://a.dev/"), known).is_err()
        );
    }

    #[test]
    fn a_file_tab_stays_inside_the_task() {
        let some = |text: &str| Some(text.to_owned());
        assert_eq!(
            checked_target(TabKind::File, None, some("docs/./README.md"), known),
            Ok(some("docs/README.md"))
        );
        for bad in ["../secret", "/etc/passwd", ".git/config", "", "."] {
            assert!(
                checked_target(TabKind::File, None, some(bad), known).is_err(),
                "{bad} must be refused"
            );
        }
        assert!(checked_target(TabKind::File, None, None, known).is_err());
    }
}
