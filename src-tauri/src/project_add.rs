//! Adding a project that does not exist on this computer yet: clone one from a URL, or start an
//! empty one. Both end in an ordinary project entry, the same as choosing an existing folder.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use brachio_git::Git;
use brachio_process::Ended;
use brachio_store::RepoRecord;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::workspace::{add_project, Database};

const PROGRESS_EVENT: &str = "project-clone-progress";
/// What a cancelled clone returns; the window that asked for the cancel ignores it.
const CANCELLED: &str = "cancelled";
const CLONE_TIME_LIMIT: Duration = Duration::from_secs(30 * 60);
const MAX_URL_LEN: usize = 2048;
const MAX_NAME_LEN: usize = 100;

/// Clones that are running, by the id the window gave them, so one can be cancelled.
#[derive(Default)]
pub struct CloneJobs(Mutex<HashMap<String, Arc<AtomicBool>>>);

impl CloneJobs {
    fn begin(&self, id: &str) -> Result<Arc<AtomicBool>, String> {
        let mut jobs = self.0.lock().map_err(|error| error.to_string())?;
        if jobs.contains_key(id) {
            return Err("This clone is already running.".to_owned());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        jobs.insert(id.to_owned(), Arc::clone(&cancel));
        Ok(cancel)
    }

    fn finish(&self, id: &str) {
        if let Ok(mut jobs) = self.0.lock() {
            jobs.remove(id);
        }
    }

    fn cancel(&self, id: &str) {
        if let Some(cancel) = self.0.lock().ok().and_then(|jobs| jobs.get(id).cloned()) {
            cancel.store(true, Ordering::Release);
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CloneProgress {
    job_id: String,
    percent: u8,
}

/// Accepts `https://` and `ssh://` URLs and the `user@host:path` form. Anything else is refused:
/// `file:` and `ext::` reach the local machine, `git://` and `http://` send code unprotected, and
/// a leading dash would be read by git as an option.
pub(crate) fn check_clone_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("Enter a repository URL.".to_owned());
    }
    if url.len() > MAX_URL_LEN || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("The URL has spaces or is too long.".to_owned());
    }
    if url.starts_with('-') {
        return Err("The URL cannot start with a dash.".to_owned());
    }
    let lower = url.to_ascii_lowercase();
    for scheme in ["https://", "ssh://"] {
        if let Some(rest) = lower.strip_prefix(scheme) {
            let host = rest.split(['/', ':']).next().unwrap_or("");
            let host = host.rsplit('@').next().unwrap_or("");
            return if host.is_empty() || !rest.contains('/') {
                Err("The URL needs a host and a repository path.".to_owned())
            } else {
                Ok(())
            };
        }
    }
    if lower.contains("://") || lower.contains("::") {
        return Err("Only https:// and ssh:// URLs can be cloned.".to_owned());
    }
    // user@host:owner/repo
    match url.split_once(':') {
        Some((host, path))
            if host.contains('@')
                && !host.contains('/')
                && !host.starts_with('@')
                && !host.ends_with('@')
                && !path.is_empty()
                && !path.starts_with('/') =>
        {
            Ok(())
        }
        _ => Err("Only https:// and ssh:// URLs can be cloned.".to_owned()),
    }
}

/// The folder a repository URL names: its last path part without `.git`.
pub(crate) fn folder_for_url(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/');
    let last = trimmed.rsplit(['/', ':']).next()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    check_folder_name(name).ok().map(|()| name.to_owned())
}

/// A single folder name that is safe on every platform.
pub(crate) fn check_folder_name(name: &str) -> Result<(), String> {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if name.trim().is_empty() {
        return Err("Enter a folder name.".to_owned());
    }
    if name.len() > MAX_NAME_LEN {
        return Err("The folder name is too long.".to_owned());
    }
    if name == "."
        || name == ".."
        || name.ends_with('.')
        || name.ends_with(' ')
        || name.starts_with(' ')
    {
        return Err("That folder name is not allowed.".to_owned());
    }
    if name.starts_with('-') {
        return Err("The folder name cannot start with a dash.".to_owned());
    }
    if name.chars().any(|c| {
        c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
    }) {
        return Err("The folder name has characters a folder cannot have.".to_owned());
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        return Err("That folder name is reserved by the system.".to_owned());
    }
    Ok(())
}

/// Where the new project goes: `name` inside the existing folder `parent`. The target must not
/// exist yet, or be an empty folder. Returns it and whether it already existed.
fn checked_target(parent: &Path, name: &str) -> Result<(PathBuf, bool), String> {
    check_folder_name(name)?;
    if !parent.is_dir() {
        return Err(format!("{} is not a folder.", parent.display()));
    }
    let target = parent.join(name);
    match std::fs::read_dir(&target) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                Err(format!(
                    "{} already exists and is not empty.",
                    target.display()
                ))
            } else {
                Ok((target, true))
            }
        }
        Err(_) if target.exists() => Err(format!(
            "{} already exists and is not a folder.",
            target.display()
        )),
        Err(_) => Ok((target, false)),
    }
}

/// Removes what a stopped clone left. A folder that existed before is emptied, not removed.
fn clean_partial(target: &Path, existed: bool) {
    if !existed {
        let _ = std::fs::remove_dir_all(target);
        return;
    }
    if let Ok(entries) = std::fs::read_dir(target) {
        for entry in entries.flatten() {
            let path = entry.path();
            let _ = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
        }
    }
}

/// Overall progress 0 to 100 from one line of `git clone --progress`: receiving objects is the
/// first 90 percent and resolving deltas the rest. Other steps are too quick to show.
fn progress_of(segment: &str) -> Option<u8> {
    let segment = segment
        .strip_prefix("remote:")
        .unwrap_or(segment)
        .trim_start();
    let (stage, rest) = segment.split_once(':')?;
    let percent: u16 = rest
        .trim_start()
        .split('%')
        .next()?
        .trim()
        .parse()
        .ok()
        .filter(|p| *p <= 100)?;
    let overall = if stage.starts_with("Receiving objects") {
        percent * 9 / 10
    } else if stage.starts_with("Resolving deltas") {
        90 + percent / 10
    } else {
        return None;
    };
    u8::try_from(overall).ok()
}

/// The reason a clone failed, from the end of what git printed: progress lines are dropped.
fn failure_text(stderr_tail: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr_tail);
    let lines: Vec<&str> = text
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.contains('%') && !line.starts_with("Cloning into"))
        .collect();
    let start = lines.len().saturating_sub(3);
    let joined = lines[start..].join("\n");
    let joined = joined.replace("fatal: ", "");
    if joined.is_empty() {
        "git clone failed.".to_owned()
    } else {
        joined
    }
}

fn clone_command(url: &str, target: &Path) -> Command {
    let mut command = Command::new("git");
    command
        // Only the two accepted transports may run, even after a redirect.
        .args(["-c", "protocol.allow=never"])
        .args(["-c", "protocol.https.allow=always"])
        .args(["-c", "protocol.ssh.allow=always"])
        // A password prompt nobody can see would hold the clone until it times out.
        .args(["-c", "core.sshCommand=ssh -o BatchMode=yes"])
        .args(["clone", "--progress", "--"])
        .arg(url)
        .arg(target)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never");
    command
}

fn run_clone(
    app: &AppHandle,
    job_id: &str,
    url: &str,
    target: &Path,
    existed: bool,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let app = app.clone();
    let id = job_id.to_owned();
    let mut pending = String::new();
    let mut last = 0u8;
    let reported = brachio_process::run_tree_reporting(
        clone_command(url, target),
        CLONE_TIME_LIMIT,
        cancel,
        move |chunk| {
            pending.push_str(&String::from_utf8_lossy(chunk));
            while let Some(end) = pending.find(['\r', '\n']) {
                let segment: String = pending.drain(..=end).collect();
                if let Some(percent) = progress_of(segment.trim()) {
                    if percent > last {
                        last = percent;
                        let _ = app.emit(
                            PROGRESS_EVENT,
                            CloneProgress {
                                job_id: id.clone(),
                                percent,
                            },
                        );
                    }
                }
            }
        },
    )
    .map_err(|error| format!("could not run git: {error}"))?;
    match reported.ended {
        Ended::Exited(status) if status.success() => Ok(()),
        Ended::Exited(_) => {
            clean_partial(target, existed);
            Err(failure_text(&reported.stderr_tail))
        }
        Ended::Cancelled => {
            clean_partial(target, existed);
            Err(CANCELLED.to_owned())
        }
        Ended::TimedOut => {
            clean_partial(target, existed);
            Err("The clone took longer than 30 minutes and was stopped.".to_owned())
        }
    }
}

/// Clones `url` into a new folder `name` (by default the one the URL names) inside `parent` and
/// adds it as a project. Progress arrives as `project-clone-progress` events carrying `job_id`;
/// `project_clone_cancel` stops it and removes the partial folder.
#[tauri::command]
pub async fn project_clone(
    app: AppHandle,
    database: State<'_, Database>,
    jobs: State<'_, CloneJobs>,
    job_id: String,
    url: String,
    parent: PathBuf,
    name: Option<String>,
) -> Result<RepoRecord, String> {
    let url = url.trim().to_owned();
    check_clone_url(&url)?;
    let name = name
        .filter(|name| !name.trim().is_empty())
        .or_else(|| folder_for_url(&url))
        .ok_or_else(|| "Enter a folder name.".to_owned())?;
    let (target, existed) = checked_target(&parent, &name)?;
    let cancel = jobs.begin(&job_id)?;
    let cloned = {
        let (app, job_id, target) = (app.clone(), job_id.clone(), target.clone());
        tauri::async_runtime::spawn_blocking(move || {
            run_clone(&app, &job_id, &url, &target, existed, &cancel)
        })
        .await
    };
    jobs.finish(&job_id);
    cloned.map_err(|error| error.to_string())??;
    add_project(&database, &target, true)
}

/// Stops a running clone. Nothing happens when the id is unknown or the clone already ended.
#[tauri::command]
pub fn project_clone_cancel(jobs: State<'_, CloneJobs>, job_id: String) {
    jobs.cancel(&job_id);
}

/// Creates the folder `name` inside `parent`, runs `git init` in it and adds it as a project. A
/// first empty commit is made when git has an identity, so tasks can branch from it right away.
#[tauri::command]
pub async fn project_init(
    database: State<'_, Database>,
    parent: PathBuf,
    name: String,
) -> Result<RepoRecord, String> {
    let name = name.trim().to_owned();
    let (target, existed) = checked_target(&parent, &name)?;
    let made = target.clone();
    tauri::async_runtime::spawn_blocking(move || init_repository(&made, existed))
        .await
        .map_err(|error| error.to_string())??;
    add_project(&database, &target, true)
}

fn init_repository(target: &Path, existed: bool) -> Result<(), String> {
    if !existed {
        std::fs::create_dir(target)
            .map_err(|error| format!("could not create {}: {error}", target.display()))?;
    }
    let git = Git::new(target);
    if let Err(error) = git.run(&["init"]) {
        if !existed {
            let _ = std::fs::remove_dir_all(target);
        }
        return Err(error.to_string());
    }
    // Without a commit there is no branch to start a task from. No identity means no commit, and
    // that is fine: the person's first commit does the same.
    let _ = git.output(&["commit", "--allow-empty", "-m", "Initial commit"]);
    Ok(())
}

/// The `origin` URL of every git project that has one, by project id, to show `owner/repo`.
#[tauri::command]
pub async fn repo_remote_urls(
    database: State<'_, Database>,
) -> Result<HashMap<String, String>, String> {
    let repos = RepoRecord::list(&*database.lock()?).map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        repos
            .into_iter()
            .filter(|repo| repo.is_git)
            .filter_map(|repo| {
                let url = Git::new(&repo.path)
                    .run(&["config", "--get", "remote.origin.url"])
                    .ok()?;
                (!url.is_empty()).then_some((repo.id, url))
            })
            .collect()
    })
    .await
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_https_ssh_and_scp_style_urls() {
        for url in [
            "https://github.com/owner/repo.git",
            "https://user@host.example/owner/repo",
            "ssh://git@github.com/owner/repo.git",
            "ssh://git@host:2222/owner/repo.git",
            "git@github.com:owner/repo.git",
            "  https://github.com/owner/repo  ",
        ] {
            assert!(check_clone_url(url).is_ok(), "{url}");
        }
    }

    #[test]
    fn refuses_other_schemes_options_and_malformed_urls() {
        for url in [
            "",
            "   ",
            "file:///etc/passwd",
            "ext::sh -c touch /tmp/x",
            "git://github.com/owner/repo",
            "http://github.com/owner/repo",
            "--upload-pack=evil",
            "-oProxyCommand=evil",
            "https://",
            "https:///owner/repo",
            "https://github.com",
            "https://github.com/owner/re po",
            "C:\\work\\repo",
            "/home/me/repo",
            "owner/repo",
            "@host:repo",
        ] {
            assert!(check_clone_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn names_the_folder_after_the_last_part_of_the_url() {
        assert_eq!(
            folder_for_url("https://github.com/owner/repo.git"),
            Some("repo".to_owned())
        );
        assert_eq!(
            folder_for_url("https://github.com/owner/repo/"),
            Some("repo".to_owned())
        );
        assert_eq!(
            folder_for_url("git@github.com:owner/my-app.git"),
            Some("my-app".to_owned())
        );
        assert_eq!(folder_for_url("https://github.com/owner/.git"), None);
    }

    #[test]
    fn refuses_folder_names_that_escape_or_break_a_platform() {
        for name in [
            "", " ", ".", "..", "a/b", "a\\b", "a:b", "-rf", "CON", "nul.txt", "x.", "a*b",
        ] {
            assert!(check_folder_name(name).is_err(), "{name:?}");
        }
        for name in ["repo", "my app", "프로젝트", "a.b-c_d", "console"] {
            assert!(check_folder_name(name).is_ok(), "{name:?}");
        }
    }

    #[test]
    fn the_target_must_be_new_or_an_empty_folder_inside_an_existing_one() {
        let parent = tempfile::tempdir().expect("temp dir");
        let (fresh, existed) = checked_target(parent.path(), "new").expect("new name");
        assert_eq!((fresh.starts_with(parent.path()), existed), (true, false));

        std::fs::create_dir(parent.path().join("empty")).expect("folder");
        assert!(
            checked_target(parent.path(), "empty")
                .expect("empty folder")
                .1
        );

        std::fs::create_dir(parent.path().join("full")).expect("folder");
        std::fs::write(parent.path().join("full").join("file"), "x").expect("file");
        assert!(checked_target(parent.path(), "full").is_err());

        std::fs::write(parent.path().join("plain"), "x").expect("file");
        assert!(checked_target(parent.path(), "plain").is_err());

        assert!(checked_target(&parent.path().join("missing"), "x").is_err());
        assert!(checked_target(parent.path(), "../escape").is_err());
    }

    #[test]
    fn progress_counts_receiving_then_resolving() {
        assert_eq!(
            progress_of("Receiving objects:  50% (50/100), 1.0 MiB | 2.0 MiB/s"),
            Some(45)
        );
        assert_eq!(
            progress_of("remote: Receiving objects: 100% (10/10), done."),
            Some(90)
        );
        assert_eq!(progress_of("Resolving deltas:  50% (5/10)"), Some(95));
        assert_eq!(
            progress_of("remote: Counting objects: 100% (5/5), done."),
            None
        );
        assert_eq!(progress_of("Cloning into 'repo'..."), None);
    }

    #[test]
    fn a_failure_shows_the_last_lines_without_progress() {
        let tail = b"Cloning into 'repo'...\r\nReceiving objects:  10% (1/10)\r\nfatal: repository 'https://x/y' not found\n";
        assert_eq!(failure_text(tail), "repository 'https://x/y' not found");
        assert_eq!(failure_text(b""), "git clone failed.");
    }

    #[test]
    fn cancelling_a_clone_job_sets_its_flag_once_and_an_unknown_id_does_nothing() {
        let jobs = CloneJobs::default();
        let flag = jobs.begin("one").expect("starts");
        assert!(jobs.begin("one").is_err());
        jobs.cancel("other");
        assert!(!flag.load(Ordering::Acquire));
        jobs.cancel("one");
        assert!(flag.load(Ordering::Acquire));
        jobs.finish("one");
        assert!(jobs.begin("one").is_ok());
    }

    #[test]
    fn a_stopped_clone_leaves_a_new_folder_removed_and_an_old_empty_one_emptied() {
        let parent = tempfile::tempdir().expect("temp dir");
        let new = parent.path().join("new");
        std::fs::create_dir_all(new.join("sub")).expect("folders");
        clean_partial(&new, false);
        assert!(!new.exists());

        let old = parent.path().join("old");
        std::fs::create_dir_all(old.join("sub")).expect("folders");
        std::fs::write(old.join("file"), "x").expect("file");
        clean_partial(&old, true);
        assert!(old.is_dir());
        assert_eq!(std::fs::read_dir(&old).expect("read").count(), 0);
    }

    #[test]
    fn init_makes_a_folder_that_is_a_git_repository() {
        let parent = tempfile::tempdir().expect("temp dir");
        let target = parent.path().join("fresh");
        init_repository(&target, false).expect("init");
        assert!(target.join(".git").exists());
        assert!(brachio_git::inspect_repo(&target).is_ok());
    }
}
