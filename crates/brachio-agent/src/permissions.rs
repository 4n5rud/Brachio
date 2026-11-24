//! The `[permissions]` section of a project's `brachio.toml`: answers Brachio's own approval bar
//! ahead of time. It never changes how an agent is launched or what an agent's own settings say.
//!
//! ```toml
//! [permissions]
//! allow = ["Read", "Grep", "Glob"]                   # read-only tools, answered without asking
//! deny = ["WebFetch", "Write:.env", "Edit:infra/"]   # a tool, or a tool and a project path
//! ```
//!
//! `Edit` also covers `MultiEdit` and `NotebookEdit`; every other tool name matches only itself.
//!
//! Anything not listed is asked, so a missing or empty section changes nothing.

use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

const CONFIG_FILE: &str = "brachio.toml";

/// Tools that can be listed under `allow`: they only read.
pub const READ_ONLY_TOOLS: [&str; 5] = ["Read", "Glob", "Grep", "LS", "NotebookRead"];

/// Tools that change a file the way `Edit` does, so a rule for `Edit` covers them too.
const EDIT_FAMILY: [&str; 3] = ["Edit", "MultiEdit", "NotebookEdit"];

fn rule_covers(rule_tool: &str, tool: &str) -> bool {
    rule_tool == "*" || rule_tool == tool || (rule_tool == "Edit" && EDIT_FAMILY.contains(&tool))
}

/// The input fields that carry a file path in an agent's tool request.
const PATH_FIELDS: [&str; 3] = ["file_path", "path", "notebook_path"];

#[derive(Default, Deserialize)]
struct Config {
    permissions: Option<Section>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Section {
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    deny: Vec<String>,
}

/// One `deny` entry: a tool name (`*` for any tool), optionally limited to a project path.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DenyRule {
    text: String,
    tool: String,
    path: Option<Vec<String>>,
}

/// What the policy says about one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    /// No rule applies: the user is asked.
    Ask,
    /// An `allow` entry answered the request.
    Allow {
        /// The `allow` entry that matched.
        rule: String,
    },
    /// A `deny` entry refused the request.
    Deny {
        /// The `deny` entry that matched.
        rule: String,
    },
}

/// A project's permission rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PermissionPolicy {
    allow: Vec<String>,
    deny: Vec<DenyRule>,
}

impl PermissionPolicy {
    /// True when no rule is set, so every request is asked.
    #[must_use]
    pub fn asks_for_everything(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty()
    }

    /// The `allow` entries as written.
    #[must_use]
    pub fn allowed(&self) -> &[String] {
        &self.allow
    }

    /// The `deny` entries as written.
    #[must_use]
    pub fn denied(&self) -> Vec<String> {
        self.deny.iter().map(|rule| rule.text.clone()).collect()
    }

    /// Parses the text of a `brachio.toml`.
    ///
    /// # Errors
    /// Fails when the file is not valid, the section has an unknown key, `allow` names a tool
    /// that can write, or a `deny` entry is malformed.
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Config = toml::from_str(text).map_err(|error| error.message().to_owned())?;
        let Some(section) = config.permissions else {
            return Ok(Self::default());
        };
        for tool in &section.allow {
            if !READ_ONLY_TOOLS.contains(&tool.as_str()) {
                return Err(format!(
                    "allow = {tool:?} is not a read-only tool. Allowed here: {}.",
                    READ_ONLY_TOOLS.join(", ")
                ));
            }
        }
        let deny = section
            .deny
            .iter()
            .map(|entry| parse_deny(entry))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            allow: section.allow,
            deny,
        })
    }

    /// Decides one request. `root` is the folder the agent works in; `input` is the request's
    /// JSON input. A deny entry wins over an allow entry. A read-only request that reaches
    /// outside `root` is never allowed automatically.
    #[must_use]
    pub fn decide(&self, tool: &str, input: &Value, root: &Path) -> PolicyDecision {
        let paths = request_paths(input, root);
        for rule in &self.deny {
            if !rule_covers(&rule.tool, tool) {
                continue;
            }
            let hit = match &rule.path {
                None => true,
                Some(wanted) => paths
                    .iter()
                    .flatten()
                    .any(|path| path.starts_with(wanted.as_slice())),
            };
            if hit {
                return PolicyDecision::Deny {
                    rule: format!("deny = {:?}", rule.text),
                };
            }
        }
        let inside = paths.iter().all(Option::is_some);
        if inside && input.is_object() && self.allow.iter().any(|allowed| allowed == tool) {
            return PolicyDecision::Allow {
                rule: format!("allow = {tool:?}"),
            };
        }
        PolicyDecision::Ask
    }
}

/// Reads `[permissions]` of `<repo_root>/brachio.toml`. A project without the file or section
/// asks for everything.
///
/// # Errors
/// Fails when the file cannot be read or the section is not valid. Callers ask for everything
/// then, and tell the user why.
pub fn read_policy(repo_root: &Path) -> Result<PermissionPolicy, String> {
    let path = repo_root.join(CONFIG_FILE);
    match fs::read_to_string(&path) {
        Ok(text) => {
            PermissionPolicy::parse(&text).map_err(|error| format!("{CONFIG_FILE}: {error}"))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(PermissionPolicy::default()),
        Err(error) => Err(format!("{CONFIG_FILE}: {error}")),
    }
}

fn parse_deny(entry: &str) -> Result<DenyRule, String> {
    let (tool, path) = match entry.split_once(':') {
        Some((tool, path)) => (tool, Some(path)),
        None => (entry, None),
    };
    let tool_ok = !tool.is_empty()
        && (tool == "*"
            || tool
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'));
    if !tool_ok {
        return Err(format!(
            "deny = {entry:?}: write a tool name, or a tool name and a path such as \"Write:.env\"."
        ));
    }
    let path = match path {
        None => None,
        Some(path) => Some(
            relative_parts(path)
                .filter(|parts| !parts.is_empty())
                .ok_or_else(|| {
                    format!(
                        "deny = {entry:?}: the path must be inside the project, such as \"infra/\"."
                    )
                })?,
        ),
    };
    Ok(DenyRule {
        text: entry.to_owned(),
        tool: tool.to_owned(),
        path,
    })
}

/// A project-relative path as components, or `None` when it climbs out of the project, is
/// absolute or has control characters.
fn relative_parts(path: &str) -> Option<Vec<String>> {
    if path.chars().any(char::is_control) || path.starts_with('/') || path.starts_with('\\') {
        return None;
    }
    if path.as_bytes().get(1) == Some(&b':') {
        return None;
    }
    normalize(path.split(['/', '\\']))
}

fn normalize<'a>(parts: impl Iterator<Item = &'a str>) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for part in parts {
        match part {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            other => out.push(fold(other)),
        }
    }
    Some(out)
}

/// Paths compare without case on Windows and macOS, where `.ENV` is the same file.
fn fold(part: &str) -> String {
    if cfg!(any(windows, target_os = "macos")) {
        part.to_lowercase()
    } else {
        part.to_owned()
    }
}

/// The paths a request names: `Some(parts)` inside the project, `None` outside it. Empty when
/// the request names no path.
fn request_paths(input: &Value, root: &Path) -> Vec<Option<Vec<String>>> {
    let root_text = root.to_string_lossy().replace('\\', "/");
    let root_parts = normalize(root_text.split('/')).unwrap_or_default();
    PATH_FIELDS
        .iter()
        .filter_map(|field| input.get(field)?.as_str())
        .map(|raw| {
            let text = raw.replace('\\', "/");
            let absolute = text.starts_with('/') || text.as_bytes().get(1) == Some(&b':');
            if absolute {
                let parts = normalize(text.split('/'))?;
                parts
                    .strip_prefix(root_parts.as_slice())
                    .map(<[String]>::to_vec)
            } else {
                normalize(text.split('/'))
            }
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn root() -> &'static Path {
        if cfg!(windows) {
            Path::new("C:/work/app")
        } else {
            Path::new("/work/app")
        }
    }

    fn abs(path: &str) -> String {
        format!("{}/{path}", root().display()).replace('\\', "/")
    }

    #[test]
    fn a_missing_or_empty_policy_asks_for_everything() {
        for text in [
            "",
            "[qa]\nprogram = \"x\"",
            "[permissions]",
            "[permissions]\nallow = []\ndeny = []",
        ] {
            let policy = PermissionPolicy::parse(text).unwrap();
            assert!(policy.asks_for_everything(), "{text}");
            for tool in ["Read", "Bash", "Write", "WebFetch"] {
                let input = json!({"file_path": "a.txt", "command": "ls"});
                assert_eq!(
                    policy.decide(tool, &input, root()),
                    PolicyDecision::Ask,
                    "{tool}"
                );
            }
        }
        let missing = tempfile::tempdir().unwrap();
        assert!(read_policy(missing.path()).unwrap().asks_for_everything());
    }

    #[test]
    fn an_edit_rule_covers_multi_edit_and_notebook_edit_but_not_write() {
        let policy = PermissionPolicy::parse(
            "[permissions]
deny = [\"Edit:infra/\"]",
        )
        .unwrap();
        let input =
            json!({"file_path": abs("infra/main.tf"), "notebook_path": abs("infra/n.ipynb")});
        for tool in ["Edit", "MultiEdit", "NotebookEdit"] {
            assert!(
                matches!(
                    policy.decide(tool, &input, root()),
                    PolicyDecision::Deny { .. }
                ),
                "{tool}"
            );
        }
        assert_eq!(policy.decide("Write", &input, root()), PolicyDecision::Ask);
    }

    #[test]
    fn a_listed_read_only_tool_is_allowed_inside_the_project_only() {
        let policy =
            PermissionPolicy::parse("[permissions]\nallow = [\"Read\", \"Grep\"]").unwrap();
        let inside = json!({"file_path": abs("src/a.rs")});
        assert!(matches!(
            policy.decide("Read", &inside, root()),
            PolicyDecision::Allow { .. }
        ));
        assert!(matches!(
            policy.decide("Grep", &json!({"pattern": "x"}), root()),
            PolicyDecision::Allow { .. }
        ));
        let outside = json!({"file_path": "/etc/passwd"});
        assert_eq!(policy.decide("Read", &outside, root()), PolicyDecision::Ask);
        let climbing = json!({"file_path": "../secret"});
        assert_eq!(
            policy.decide("Read", &climbing, root()),
            PolicyDecision::Ask
        );
        assert_eq!(
            policy.decide("Glob", &json!({}), root()),
            PolicyDecision::Ask
        );
    }

    #[test]
    fn a_tool_that_can_write_cannot_be_allowed() {
        for tool in ["Bash", "Write", "Edit", "WebFetch"] {
            let text = format!("[permissions]\nallow = [\"{tool}\"]");
            assert!(PermissionPolicy::parse(&text).is_err(), "{tool}");
        }
    }

    #[test]
    fn deny_refuses_a_tool_or_a_path_and_beats_allow() {
        let text = "[permissions]\nallow = [\"Read\"]\ndeny = [\"WebFetch\", \"*:.env\", \"Edit:infra/\", \"Read:secrets\"]";
        let policy = PermissionPolicy::parse(text).unwrap();
        let denied = |tool: &str, input: Value| {
            matches!(
                policy.decide(tool, &input, root()),
                PolicyDecision::Deny { .. }
            )
        };
        assert!(denied("WebFetch", json!({"url": "x"})));
        assert!(denied("Write", json!({"file_path": abs(".env")})));
        assert!(denied("Edit", json!({"file_path": "infra/main.tf"})));
        assert!(!denied("Edit", json!({"file_path": "src/infra/main.tf"})));
        assert!(denied("Read", json!({"file_path": "secrets/key"})));
        assert!(denied("Read", json!({"file_path": "src/../secrets/key"})));
    }

    #[test]
    fn a_mistake_in_the_section_is_an_error_not_a_silent_allow() {
        for text in [
            "[permissions]\nalow = [\"Read\"]",
            "[permissions]\ndeny = [\"Write:\"]",
            "[permissions]\ndeny = [\"Write:../x\"]",
            "[permissions]\ndeny = [\"Write:/etc\"]",
            "[permissions]\ndeny = [\"\"]",
            "[permissions\nallow = 1",
        ] {
            assert!(PermissionPolicy::parse(text).is_err(), "{text}");
        }
    }
}
