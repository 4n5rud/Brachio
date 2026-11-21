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
    helper_folder: Option<PathBuf>,
    /// A plugin folder that teaches Claude Code the Brachio skills, added per session.
    plugin: Option<PathBuf>,
}

const SEND_SKILL: &str = include_str!("../skills/brachio-send/SKILL.md");
const LEAD_SKILL: &str = include_str!("../skills/brachio-lead/SKILL.md");
/// Every agent tab gets both: a task cannot tell in advance whether it will lead or be led.
const SKILLS: [(&str, &str); 2] = [("brachio-send", SEND_SKILL), ("brachio-lead", LEAD_SKILL)];

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
        let helper_folder = helper_beside_app()
            .ok()
            .flatten()
            .and_then(|helper| helper.parent().map(Path::to_path_buf));
        let plugin = helper_folder.as_ref().and_then(|_| {
            write_plugin(folder)
                .map_err(|error| tracing::info!(%error, "the Brachio skills are off"))
                .ok()
        });
        Self {
            endpoint,
            settings,
            helper_folder,
            plugin,
        }
    }

    /// What an agent needs to use `brachio send` and `brachio check`: where the app listens, who
    /// the agent is, and the helper on its `PATH`. Empty when the helper is not installed.
    pub fn mail_environment(&self, key: &str) -> Vec<(String, String)> {
        let Some(folder) = &self.helper_folder else {
            return Vec::new();
        };
        let mut folders = vec![folder.clone()];
        if let Some(existing) = env::var_os("PATH") {
            folders.extend(env::split_paths(&existing));
        }
        let mut variables = vec![
            (ENDPOINT_VARIABLE.to_owned(), self.endpoint.clone()),
            (KEY_VARIABLE.to_owned(), key.to_owned()),
        ];
        if let Ok(path) = env::join_paths(folders) {
            variables.push(("PATH".to_owned(), path.to_string_lossy().into_owned()));
        }
        variables
    }

    /// Arguments that add the hook to a Claude Code session.
    pub fn claude_arguments(&self) -> Vec<String> {
        let mut arguments = Vec::new();
        if let Some(path) = &self.settings {
            arguments.extend(["--settings".to_owned(), path.to_string_lossy().into_owned()]);
        }
        if let Some(path) = &self.plugin {
            arguments.extend([
                "--plugin-dir".to_owned(),
                path.to_string_lossy().into_owned(),
            ]);
        }
        arguments
    }

    /// Arguments that give a Codex session the same skills as developer instructions. Codex has
    /// no plugin folder, and a project `AGENTS.md` would change the user's repository. Empty when
    /// the helper is not installed.
    pub fn codex_arguments(&self) -> Vec<String> {
        if self.helper_folder.is_none() {
            return Vec::new();
        }
        let text = SKILLS
            .iter()
            .map(|(_, skill)| skill_body(skill))
            .collect::<Vec<_>>()
            .join("\n\n");
        // Codex reads the value as TOML; a JSON string is a valid TOML basic string.
        vec![
            "-c".to_owned(),
            format!("developer_instructions={}", serde_json::Value::from(text)),
        ]
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

/// A skill without its front matter.
fn skill_body(skill: &str) -> String {
    let text = skill.replace("\r\n", "\n");
    match text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
    {
        Some((_, body)) => body.trim().to_owned(),
        None => text.trim().to_owned(),
    }
}

/// Writes the plugin that carries the Brachio skills and returns its folder.
fn write_plugin(folder: &Path) -> Result<PathBuf> {
    let plugin = folder.join("plugin");
    fs::create_dir_all(plugin.join(".claude-plugin"))?;
    fs::write(
        plugin.join(".claude-plugin").join("plugin.json"),
        serde_json::json!({
            "name": "brachio",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Lets agents in Brachio tasks message each other and split work into child tasks."
        })
        .to_string(),
    )?;
    for (name, text) in SKILLS {
        let skill = plugin.join("skills").join(name);
        fs::create_dir_all(&skill).with_context(|| format!("create {}", skill.display()))?;
        fs::write(skill.join("SKILL.md"), text)?;
    }
    Ok(plugin)
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
    let mail = serde_json::json!([{
        "matcher": "*",
        "hooks": [{
            "type": "command",
            "command": format!("\"{helper}\" hook mail"),
            "timeout": 10
        }]
    }]);
    serde_json::json!({
        "hooks": {
            "PostToolUse": mail,
            "UserPromptSubmit": mail,
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
    use super::{settings_json, write_plugin, HookSetup, SKILLS};
    use std::path::Path;

    #[test]
    fn each_skill_is_a_short_plugin_skill_with_a_name_and_a_one_line_description() {
        let folder = tempfile::tempdir().expect("temp dir");
        let plugin = write_plugin(folder.path()).expect("write plugin");
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(plugin.join(".claude-plugin/plugin.json")).expect("manifest"),
        )
        .expect("json");
        assert_eq!(manifest["name"], "brachio");
        for (name, text) in SKILLS {
            let skill = std::fs::read_to_string(plugin.join(format!("skills/{name}/SKILL.md")))
                .expect("skill");
            assert_eq!(skill, text);
            // A Windows checkout may turn the shipped file's line ends into CRLF.
            assert!(skill
                .replace("\r\n", "\n")
                .starts_with(&format!("---\nname: {name}\ndescription: ")));
            assert!(
                skill.lines().count() <= 40,
                "{name}: {} lines",
                skill.lines().count()
            );
        }
    }

    #[test]
    fn codex_gets_both_skills_as_one_toml_string_of_developer_instructions() {
        let setup = HookSetup {
            endpoint: String::new(),
            settings: None,
            helper_folder: Some(Path::new("helper").to_path_buf()),
            plugin: None,
        };
        let arguments = setup.codex_arguments();
        assert_eq!(arguments[0], "-c");
        let value = arguments[1]
            .strip_prefix("developer_instructions=")
            .expect("key");
        let text: String = serde_json::from_str(value).expect("a quoted string");
        assert!(text.contains("brachio task create") && text.contains("brachio peers"));
        assert!(!text.contains("name: brachio-"), "front matter is left out");
        let without_helper = HookSetup {
            helper_folder: None,
            ..setup
        };
        assert!(without_helper.codex_arguments().is_empty());
    }

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
        for event in ["PostToolUse", "UserPromptSubmit"] {
            assert_eq!(
                settings["hooks"][event][0]["hooks"][0]["command"],
                "\"C:/Program Files/Brachio/brachio.exe\" hook mail"
            );
        }
    }
}
