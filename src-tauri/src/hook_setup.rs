//! Giving Claude Code a hook that asks the app, without touching the user's own settings.
//!
//! The hook is added for one session with `claude --settings <file>`, which Claude Code merges
//! with the user's settings. The file points at the `brachio` helper next to the app. Without the
//! helper nothing is added and the agent asks in the terminal as before.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use brachio_runtime::approval::{DEFAULT_WAIT, ENDPOINT_VARIABLE, KEY_VARIABLE};

/// What the terminals of Claude Code tasks need for in-app approval.
pub struct HookSetup {
    endpoint: String,
    settings: Option<PathBuf>,
}

impl HookSetup {
    /// Writes the settings file under `folder` when the helper is installed. A failure only
    /// switches approval in the app off.
    pub fn prepare(folder: &Path, endpoint: String) -> Self {
        let settings = match write_settings(folder) {
            Ok(path) => path,
            Err(error) => {
                tracing::info!(%error, "in-app approval is off");
                None
            }
        };
        Self { endpoint, settings }
    }

    /// Arguments that add the hook to a Claude Code session.
    pub fn claude_arguments(&self) -> Vec<String> {
        self.settings.as_ref().map_or_else(Vec::new, |path| {
            vec!["--settings".to_owned(), path.to_string_lossy().into_owned()]
        })
    }

    /// Environment that tells the hook where to ask and which terminal it belongs to.
    pub fn environment(&self, key: &str) -> Vec<(String, String)> {
        if self.settings.is_none() {
            return Vec::new();
        }
        vec![
            (ENDPOINT_VARIABLE.to_owned(), self.endpoint.clone()),
            (KEY_VARIABLE.to_owned(), key.to_owned()),
        ]
    }
}

fn write_settings(folder: &Path) -> Result<Option<PathBuf>> {
    let Some(helper) = helper_beside_app()? else {
        return Ok(None);
    };
    fs::create_dir_all(folder).with_context(|| format!("create {}", folder.display()))?;
    let path = folder.join("claude-settings.json");
    fs::write(&path, settings_json(&helper))?;
    Ok(Some(path))
}

fn helper_beside_app() -> Result<Option<PathBuf>> {
    let name = if cfg!(windows) {
        "brachio.exe"
    } else {
        "brachio"
    };
    Ok(env::current_exe()?
        .parent()
        .map(|folder| folder.join(name))
        .filter(|path| path.exists()))
}

/// Claude Code runs hook commands through a shell, where backslashes would be taken as escapes,
/// so the path is written with forward slashes and quoted.
fn settings_json(helper: &Path) -> String {
    let helper = helper.to_string_lossy().replace('\\', "/");
    serde_json::json!({
        "hooks": {
            "PermissionRequest": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": format!("\"{helper}\" hook permission-request"),
                    // A little longer than the helper's own wait.
                    "timeout": DEFAULT_WAIT.as_secs() + 10
                }]
            }]
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::settings_json;
    use std::path::Path;

    #[test]
    fn the_hook_command_quotes_the_helper_and_uses_forward_slashes() {
        let settings: serde_json::Value = serde_json::from_str(&settings_json(Path::new(
            "C:\\Program Files\\Brachio\\brachio.exe",
        )))
        .expect("json");
        let hook = &settings["hooks"]["PermissionRequest"][0];
        assert_eq!(hook["matcher"], "*");
        assert_eq!(
            hook["hooks"][0]["command"],
            "\"C:/Program Files/Brachio/brachio.exe\" hook permission-request"
        );
        assert_eq!(hook["hooks"][0]["timeout"], 310);
    }
}
