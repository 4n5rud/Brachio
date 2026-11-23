//! Agent presets a repository lists in its `brachio.toml`:
//!
//! ```toml
//! [profiles.careful]
//! agent = "claude_code"
//! model = "sonnet"
//! args = ["--max-turns", "20"]
//! env = { TEAM = "docs" }
//! ```
//!
//! A profile can only choose from a short allowlist of options. Anything that changes
//! permissions or approvals is refused, so a profile can never switch the agent's approval
//! prompts off.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::AgentKind;

/// The file the profiles are read from.
pub const CONFIG_FILE: &str = "brachio.toml";

/// Longest model name.
const MAX_MODEL_CHARS: usize = 64;
/// Longest environment value.
const MAX_ENV_VALUE_CHARS: usize = 256;
/// Most extra arguments one profile may carry.
const MAX_ARGS: usize = 8;

/// Words that mean a setting skips or weakens approvals. Refused anywhere in a profile, and
/// checked again by the test that covers every launch.
pub const BYPASS_WORDS: [&str; 9] = [
    "dangerously",
    "bypass",
    "yolo",
    "skip-permissions",
    "allow-all",
    "full-auto",
    "approval",
    "permission",
    "sandbox",
];

/// Why a profile cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProfileError {
    /// `brachio.toml` is not valid TOML.
    #[error("brachio.toml is not valid: {0}")]
    Invalid(String),
    /// The profile asks for something that is not allowed.
    #[error("{0}")]
    Refused(String),
}

/// What a profile holds, as written.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProfile {
    agent: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

#[derive(Default, Deserialize)]
struct RepoProfiles {
    #[serde(default)]
    profiles: BTreeMap<String, toml::Value>,
}

/// A checked profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProfile {
    /// The name after `profiles.`.
    pub name: String,
    /// The agent it is for.
    pub agent: AgentKind,
    /// Model passed as `--model`.
    pub model: Option<String>,
    /// Extra arguments, all from the allowlist.
    pub args: Vec<String>,
    /// Extra environment, by name.
    pub env: Vec<(String, String)>,
}

/// A profile of a repository, usable or with the reason it is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileEntry {
    /// The name after `profiles.`.
    pub name: String,
    /// The checked profile, or why it was refused.
    pub profile: Result<AgentProfile, ProfileError>,
}

impl AgentProfile {
    /// Arguments to add to the agent's command line.
    #[must_use]
    pub fn launch_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(model) = &self.model {
            args.extend(["--model".to_owned(), model.clone()]);
        }
        args.extend(self.args.iter().cloned());
        args
    }
}

fn mentions_bypass(text: &str) -> bool {
    let lower = text.to_lowercase();
    BYPASS_WORDS.iter().any(|word| lower.contains(word))
}

fn is_number(text: &str) -> bool {
    !text.is_empty() && text.len() <= 4 && text.chars().all(|c| c.is_ascii_digit())
}

/// Checks `args` against the allowlist of `agent`: flags that take no value, and flags that
/// take a number.
fn check_args(agent: AgentKind, args: &[String]) -> Result<(), ProfileError> {
    let (plain, numeric): (&[&str], &[&str]) = match agent {
        AgentKind::ClaudeCode => (&["--verbose"], &["--max-turns"]),
        AgentKind::Codex => (&["--search"], &[]),
    };
    if args.len() > MAX_ARGS {
        return Err(ProfileError::Refused(format!(
            "at most {MAX_ARGS} args are allowed"
        )));
    }
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        if mentions_bypass(arg) {
            return Err(ProfileError::Refused(format!(
                "{arg:?} changes approvals or permissions and cannot be used in a profile"
            )));
        }
        if plain.contains(&arg) {
            index += 1;
        } else if numeric.contains(&arg) {
            match args.get(index + 1) {
                Some(value) if is_number(value) => index += 2,
                _ => {
                    return Err(ProfileError::Refused(format!("{arg} needs a number")));
                }
            }
        } else {
            return Err(ProfileError::Refused(format!(
                "{arg:?} is not an allowed arg for this agent"
            )));
        }
    }
    Ok(())
}

fn check_env(env: &BTreeMap<String, String>) -> Result<Vec<(String, String)>, ProfileError> {
    let mut checked = Vec::new();
    for (name, value) in env {
        let valid_name = !name.is_empty()
            && name.len() <= 64
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            && !name.starts_with(|c: char| c.is_ascii_digit());
        if !valid_name {
            return Err(ProfileError::Refused(format!(
                "{name:?} is not a valid variable name; use capital letters, digits and _"
            )));
        }
        let reserved = name.starts_with("BRACHIO_")
            || matches!(
                name.as_str(),
                "PATH" | "HOME" | "USERPROFILE" | "SHELL" | "COMSPEC" | "PATHEXT"
            )
            || name.starts_with("LD_")
            || name.starts_with("DYLD_");
        if reserved || mentions_bypass(name) {
            return Err(ProfileError::Refused(format!(
                "{name} cannot be set by a profile"
            )));
        }
        if value.chars().count() > MAX_ENV_VALUE_CHARS || value.chars().any(char::is_control) {
            return Err(ProfileError::Refused(format!(
                "the value of {name} must be one short line"
            )));
        }
        if mentions_bypass(value) {
            return Err(ProfileError::Refused(format!(
                "the value of {name} changes approvals or permissions"
            )));
        }
        checked.push((name.clone(), value.clone()));
    }
    Ok(checked)
}

fn check(name: &str, raw: RawProfile) -> Result<AgentProfile, ProfileError> {
    let valid_name = !name.is_empty()
        && name.len() <= 40
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid_name {
        return Err(ProfileError::Refused(
            "a profile name uses letters, digits, - and _ (at most 40)".to_owned(),
        ));
    }
    let agent = match raw.agent.as_str() {
        "claude_code" => AgentKind::ClaudeCode,
        "codex" => AgentKind::Codex,
        other => {
            return Err(ProfileError::Refused(format!(
                "unknown agent {other:?}; use claude_code or codex"
            )))
        }
    };
    if let Some(model) = &raw.model {
        let valid = !model.is_empty()
            && model.chars().count() <= MAX_MODEL_CHARS
            && !model.starts_with('-')
            && model.chars().all(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '[' | ']')
            });
        if !valid || mentions_bypass(model) {
            return Err(ProfileError::Refused(format!(
                "{model:?} is not an allowed model name"
            )));
        }
    }
    check_args(agent, &raw.args)?;
    Ok(AgentProfile {
        name: name.to_owned(),
        agent,
        model: raw.model,
        args: raw.args,
        env: check_env(&raw.env)?,
    })
}

/// The profiles in the text of a `brachio.toml`, sorted by name. A profile that is refused is
/// listed with its reason; the others stay usable.
///
/// # Errors
/// Fails when the text is not valid TOML.
pub fn parse_profiles(text: &str) -> Result<Vec<ProfileEntry>, ProfileError> {
    let parsed: RepoProfiles =
        toml::from_str(text).map_err(|error| ProfileError::Invalid(error.message().to_owned()))?;
    Ok(parsed
        .profiles
        .into_iter()
        .map(|(name, value)| {
            let profile = value
                .try_into::<RawProfile>()
                .map_err(|error| ProfileError::Refused(error.message().to_owned()))
                .and_then(|raw| check(&name, raw));
            ProfileEntry { name, profile }
        })
        .collect())
}

/// The profiles of the repository at `repo_root`; none when it has no `brachio.toml`.
///
/// # Errors
/// Fails when the file cannot be read or is not valid TOML.
pub fn read_profiles(repo_root: &Path) -> Result<Vec<ProfileEntry>, ProfileError> {
    match std::fs::read_to_string(repo_root.join(CONFIG_FILE)) {
        Ok(text) => parse_profiles(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(ProfileError::Invalid(error.to_string())),
    }
}

/// One usable profile by name.
///
/// # Errors
/// Fails when the file cannot be read, the profile does not exist, or it is refused.
pub fn find_profile(repo_root: &Path, name: &str) -> Result<AgentProfile, ProfileError> {
    read_profiles(repo_root)?
        .into_iter()
        .find(|entry| entry.name == name)
        .ok_or_else(|| ProfileError::Refused(format!("profile {name:?} no longer exists")))?
        .profile
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn one(text: &str) -> Result<AgentProfile, ProfileError> {
        parse_profiles(text).unwrap().remove(0).profile
    }

    #[test]
    fn a_profile_picks_agent_model_allowed_args_and_env() {
        let profile = one(
            "[profiles.careful]\nagent = \"claude_code\"\nmodel = \"sonnet\"\n\
             args = [\"--max-turns\", \"20\", \"--verbose\"]\nenv = { TEAM = \"docs\" }\n",
        )
        .unwrap();
        assert_eq!(profile.agent, AgentKind::ClaudeCode);
        assert_eq!(
            profile.launch_args(),
            ["--model", "sonnet", "--max-turns", "20", "--verbose"]
        );
        assert_eq!(profile.env, [("TEAM".to_owned(), "docs".to_owned())]);
    }

    #[test]
    fn bypass_flags_models_and_env_are_refused() {
        for args in [
            "\"--dangerously-skip-permissions\"",
            "\"--dangerously-bypass-approvals-and-sandbox\"",
            "\"--yolo\"",
            "\"--permission-mode\", \"bypassPermissions\"",
            "\"-c\", \"approval_policy=never\"",
            "\"--full-auto\"",
            "\"--max-turns\", \"x\"",
        ] {
            for agent in ["claude_code", "codex"] {
                let text = format!("[profiles.p]\nagent = \"{agent}\"\nargs = [{args}]\n");
                assert!(one(&text).is_err(), "{agent} {args}");
            }
        }
        assert!(one("[profiles.p]\nagent = \"codex\"\nmodel = \"--yolo\"\n").is_err());
        assert!(one("[profiles.p]\nagent = \"codex\"\nmodel = \"x --y\"\n").is_err());
        assert!(one("[profiles.p]\nagent = \"codex\"\nenv = { PATH = \"/x\" }\n").is_err());
        assert!(one("[profiles.p]\nagent = \"codex\"\nenv = { BRACHIO_PORT = \"1\" }\n").is_err());
        assert!(one("[profiles.p]\nagent = \"codex\"\nenv = { A = \"--yolo\" }\n").is_err());
        assert!(one("[profiles.p]\nagent = \"gemini\"\n").is_err());
        assert!(one("[profiles.p]\nagent = \"codex\"\nsandbox = \"off\"\n").is_err());
    }

    #[test]
    fn a_refused_profile_does_not_hide_the_others() {
        let entries = parse_profiles(
            "[profiles.bad]\nagent = \"codex\"\nargs = [\"--yolo\"]\n\
             [profiles.good]\nagent = \"codex\"\n",
        )
        .unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].profile.is_err());
        assert!(entries[1].profile.is_ok());
    }

    #[test]
    fn a_file_without_profiles_or_with_only_setup_has_none() {
        assert!(parse_profiles("[setup]\ncopy = [\".env\"]\n")
            .unwrap()
            .is_empty());
        assert!(parse_profiles("[profiles\n").is_err());
    }
}
