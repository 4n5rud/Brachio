//! Agent profiles from a repository's `brachio.toml`: listing them for the new-task dialog,
//! and adding a task's profile to its agent's launch.

use brachio_agent::{find_profile, read_profiles, AgentKind, ProfileEntry};
use brachio_store::{join_compare_group, task_profile, RepoRecord};
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::workspace::Database;

/// A profile as the dialog shows it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    pub name: String,
    /// `claude_code` or `codex`; empty when the profile was refused.
    pub agent: String,
    pub model: Option<String>,
    pub args: Vec<String>,
    /// Names only; values stay in the file.
    pub env: Vec<String>,
    /// Why the profile cannot be used.
    pub error: Option<String>,
}

fn view(entry: ProfileEntry) -> ProfileView {
    match entry.profile {
        Ok(profile) => ProfileView {
            name: entry.name,
            agent: profile.agent.spec().id.to_owned(),
            model: profile.model,
            args: profile.args,
            env: profile.env.into_iter().map(|(name, _)| name).collect(),
            error: None,
        },
        Err(error) => ProfileView {
            name: entry.name,
            agent: String::new(),
            model: None,
            args: Vec::new(),
            env: Vec::new(),
            error: Some(error.to_string()),
        },
    }
}

/// The profiles in the repository's `brachio.toml`, with the reason for any that is refused.
#[tauri::command]
pub fn repo_profiles(
    database: State<'_, Database>,
    repo_id: String,
) -> Result<Vec<ProfileView>, String> {
    let repo = RepoRecord::get(&*database.lock()?, &repo_id).map_err(|e| e.to_string())?;
    read_profiles(&repo.path)
        .map(|entries| entries.into_iter().map(view).collect())
        .map_err(|e| e.to_string())
}

/// Puts the task in a comparison group unless it is in one already, and returns the group.
#[tauri::command]
pub fn task_join_compare_group(
    database: State<'_, Database>,
    task_id: String,
    group: String,
) -> Result<String, String> {
    join_compare_group(&*database.lock()?, &task_id, &group).map_err(|e| e.to_string())
}

/// Arguments and environment a profile adds to a launch.
pub(crate) struct LaunchAdditions {
    pub arguments: Vec<String>,
    pub environment: Vec<(String, String)>,
}

/// What a task's profile adds to the launch of `kind`: arguments and environment. Nothing when
/// the task has no profile or the profile is for the other agent. A profile that was removed or
/// is no longer allowed stops the launch instead of starting with other settings.
pub(crate) fn launch_additions(
    connection: &Connection,
    task_id: &str,
    repo: &RepoRecord,
    kind: AgentKind,
) -> Result<Option<LaunchAdditions>, String> {
    let Some(name) = task_profile(connection, task_id).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let profile = find_profile(&repo.path, &name)
        .map_err(|error| format!("The task's profile {name:?} cannot be used: {error}"))?;
    if profile.agent != kind {
        return Ok(None);
    }
    Ok(Some(LaunchAdditions {
        arguments: profile.launch_args(),
        environment: profile.env,
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use brachio_agent::{parse_profiles, BYPASS_WORDS};

    use super::*;

    #[test]
    fn a_profile_with_every_allowed_option_adds_no_bypass_flag_to_a_launch() {
        let text = "[profiles.c]\nagent = \"claude_code\"\nmodel = \"sonnet\"\n\
                    args = [\"--verbose\", \"--max-turns\", \"5\"]\nenv = { TEAM = \"x\" }\n\
                    [profiles.x]\nagent = \"codex\"\nmodel = \"gpt-5\"\nargs = [\"--search\"]\n";
        for entry in parse_profiles(text).unwrap() {
            let profile = entry.profile.unwrap();
            let kind = profile.agent;
            let mut launch = kind.interactive_with_prompt("do it");
            let mut args = profile.launch_args();
            args.append(&mut launch.args);
            args.extend(
                profile
                    .env
                    .iter()
                    .map(|(name, value)| format!("{name}={value}")),
            );
            for argument in &args {
                let lower = argument.to_lowercase();
                for word in BYPASS_WORDS {
                    assert!(!lower.contains(word), "{word} in {argument:?}");
                }
            }
        }
    }

    #[test]
    fn the_dialog_lists_a_refused_profile_with_its_reason_and_no_settings() {
        let entries =
            parse_profiles("[profiles.bad]\nagent = \"codex\"\nargs = [\"--yolo\"]\n").unwrap();
        let shown = view(entries.into_iter().next().unwrap());
        assert!(shown.error.is_some_and(|e| e.contains("--yolo")));
        assert!(shown.args.is_empty() && shown.agent.is_empty());
    }

    #[test]
    fn a_task_launches_with_its_profile_only_for_that_agent_and_never_with_a_bad_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().display().to_string().replace('\\', "/");
        let db = brachio_store::open_in_memory().unwrap();
        db.execute_batch(&format!(
            "INSERT INTO repos (id, path, default_branch) VALUES ('r', '{path}', 'main');
             INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
             VALUES ('t', 'r', 'T', 'p', 'b', '/w', 'new', 0);"
        ))
        .unwrap();
        let repo = RepoRecord::get(&db, "r").unwrap();
        assert!(launch_additions(&db, "t", &repo, AgentKind::Codex)
            .unwrap()
            .is_none());
        brachio_store::set_task_profile(&db, "t", "fast").unwrap();
        std::fs::write(
            dir.path().join("brachio.toml"),
            "[profiles.fast]
agent = \"codex\"
model = \"gpt-5\"
env = { A = \"1\" }
",
        )
        .unwrap();
        let added = launch_additions(&db, "t", &repo, AgentKind::Codex)
            .unwrap()
            .unwrap();
        assert_eq!(added.arguments, ["--model", "gpt-5"]);
        assert_eq!(added.environment, [("A".to_owned(), "1".to_owned())]);
        assert!(launch_additions(&db, "t", &repo, AgentKind::ClaudeCode)
            .unwrap()
            .is_none());
        std::fs::write(
            dir.path().join("brachio.toml"),
            "[profiles.fast]
agent = \"codex\"
args = [\"--yolo\"]
",
        )
        .unwrap();
        assert!(launch_additions(&db, "t", &repo, AgentKind::Codex).is_err());
        std::fs::remove_file(dir.path().join("brachio.toml")).unwrap();
        assert!(launch_additions(&db, "t", &repo, AgentKind::Codex).is_err());
    }
}
