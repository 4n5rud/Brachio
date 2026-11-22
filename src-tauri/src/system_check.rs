//! Facts the first-run checklist needs that the window cannot read itself.

use std::process::{Command, Stdio};

use serde::Serialize;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemFacts {
    /// First line of `git --version`, or `None` when git cannot run.
    git: Option<String>,
    /// Version of the web view that draws the window.
    webview: Option<String>,
}

fn git_version() -> Option<String> {
    let mut command = Command::new("git");
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().ok().filter(|out| out.status.success())?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().next().map(|line| line.trim().to_owned())
}

/// Reads the git and web view versions.
#[tauri::command]
pub async fn system_check() -> SystemFacts {
    let git = tokio::task::spawn_blocking(git_version)
        .await
        .unwrap_or_default();
    SystemFacts {
        git,
        webview: tauri::webview_version().ok(),
    }
}
