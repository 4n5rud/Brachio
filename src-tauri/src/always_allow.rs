//! "Always allow in this task": remembered answers to tool requests, kept in memory only.
//!
//! A rule names one task and one tool, and is one of two kinds. An exact rule covers one target:
//! the whole command for `Bash`, the exact file path for file edits. A prefix rule covers every
//! single simple `Bash` command that starts with the same few words, such as `gh pr merge`; it is
//! offered only for the programs and actions listed in `command_words`. A rule lives until the app
//! closes or the user revokes it. Only tool requests from an agent can match; creating tasks and
//! QA runs never do.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::command_words::{offered_prefix, prefix_matches, simple_words};

/// How much a remembered answer covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// Exactly the command or file path in `target`.
    Exact,
    /// Any single simple command that starts with the words in `target`.
    Prefix,
}

/// A remembered answer, shown in the task's Inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: u64,
    pub tool: String,
    pub kind: RuleKind,
    /// The exact command or file path, or the words of a prefix rule joined by spaces.
    pub target: String,
    /// The words a prefix rule matches; empty for an exact rule.
    #[serde(skip)]
    words: Vec<String>,
}

/// What a request would have to match to be allowed again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub tool: String,
    pub target: String,
    /// The words of the command when it is a single simple command; always `None` for other tools.
    words: Option<Vec<String>>,
    /// The leading words a prefix rule could cover, when the program and action are listed.
    prefix: Option<Vec<String>>,
}

impl Candidate {
    /// The prefix a rule could cover, joined by spaces, for the label of "always allow".
    #[must_use]
    pub fn prefix_label(&self) -> Option<String> {
        self.prefix.as_ref().map(|words| words.join(" "))
    }
}

/// Which input fields a tool may carry for a request to be rememberable. A request with any other
/// field (a sandbox override, a background flag) is never matched, so a rule cannot widen.
fn allowed_fields(tool: &str) -> Option<(&'static str, &'static [&'static str])> {
    match tool {
        "Bash" => Some(("command", &["command", "description", "timeout"])),
        "Edit" => Some((
            "file_path",
            &["file_path", "old_string", "new_string", "replace_all"],
        )),
        "Write" => Some(("file_path", &["file_path", "content"])),
        "MultiEdit" => Some(("file_path", &["file_path", "edits"])),
        _ => None,
    }
}

/// The tool and exact target of a request, or `None` when it cannot be remembered.
#[must_use]
pub fn candidate(tool: &str, input: &Value) -> Option<Candidate> {
    let (target_field, fields) = allowed_fields(tool)?;
    let object: &Map<String, Value> = input.as_object()?;
    if object.keys().any(|key| !fields.contains(&key.as_str())) {
        return None;
    }
    let target = object.get(target_field)?.as_str()?;
    if target.is_empty() {
        return None;
    }
    let words = (tool == "Bash").then(|| simple_words(target)).flatten();
    let prefix = words.as_deref().and_then(offered_prefix);
    Some(Candidate {
        tool: tool.to_owned(),
        target: target.to_owned(),
        words,
        prefix,
    })
}

/// The task a terminal key belongs to.
#[must_use]
pub fn task_of(key: &str) -> &str {
    key.split(':').next().unwrap_or(key)
}

#[derive(Default)]
pub struct AlwaysAllow {
    rules: Mutex<HashMap<String, Vec<Rule>>>,
    next: Mutex<u64>,
}

impl AlwaysAllow {
    /// Remembers exactly this tool and target for the task. The same rule is stored once.
    pub fn add(&self, task_id: &str, candidate: &Candidate) -> Option<Rule> {
        self.store(task_id, candidate, RuleKind::Exact)
    }

    /// Remembers the candidate's prefix for the task. `None` when it has none to offer.
    pub fn add_prefix(&self, task_id: &str, candidate: &Candidate) -> Option<Rule> {
        candidate.prefix.as_ref()?;
        self.store(task_id, candidate, RuleKind::Prefix)
    }

    fn store(&self, task_id: &str, candidate: &Candidate, kind: RuleKind) -> Option<Rule> {
        let (target, words) = match kind {
            RuleKind::Exact => (candidate.target.clone(), Vec::new()),
            RuleKind::Prefix => {
                let words = candidate.prefix.clone()?;
                (words.join(" "), words)
            }
        };
        let mut rules = self.rules.lock().ok()?;
        let list = rules.entry(task_id.to_owned()).or_default();
        if let Some(existing) = list
            .iter()
            .find(|rule| rule.tool == candidate.tool && rule.kind == kind && rule.target == target)
        {
            return Some(existing.clone());
        }
        let mut next = self.next.lock().ok()?;
        *next += 1;
        let rule = Rule {
            id: *next,
            tool: candidate.tool.clone(),
            kind,
            target,
            words,
        };
        list.push(rule.clone());
        Some(rule)
    }

    /// True when the task has a rule that covers this request: an exact rule for the same tool
    /// and target, or a prefix rule whose words start a single simple command.
    pub fn allows(&self, task_id: &str, candidate: &Candidate) -> bool {
        self.rules.lock().is_ok_and(|rules| {
            rules.get(task_id).is_some_and(|list| {
                list.iter().any(|rule| {
                    rule.tool == candidate.tool
                        && match rule.kind {
                            RuleKind::Exact => rule.target == candidate.target,
                            RuleKind::Prefix => candidate
                                .words
                                .as_deref()
                                .is_some_and(|words| prefix_matches(&rule.words, words)),
                        }
                })
            })
        })
    }

    pub fn list(&self, task_id: &str) -> Vec<Rule> {
        self.rules
            .lock()
            .map(|rules| rules.get(task_id).cloned().unwrap_or_default())
            .unwrap_or_default()
    }

    /// Removes one rule of the task. `false` when it was already gone.
    pub fn revoke(&self, task_id: &str, rule_id: u64) -> bool {
        let Ok(mut rules) = self.rules.lock() else {
            return false;
        };
        let Some(list) = rules.get_mut(task_id) else {
            return false;
        };
        let before = list.len();
        list.retain(|rule| rule.id != rule_id);
        list.len() != before
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn bash(command: &str) -> Candidate {
        candidate("Bash", &json!({ "command": command })).expect("candidate")
    }

    #[test]
    fn only_the_exact_command_of_the_same_task_matches() {
        let rules = AlwaysAllow::default();
        rules.add("t1", &bash("cargo test"));
        assert!(rules.allows("t1", &bash("cargo test")));
        assert!(!rules.allows("t1", &bash("cargo test --all")));
        assert!(!rules.allows("t1", &bash("cargo test ")));
        assert!(!rules.allows("t2", &bash("cargo test")));
    }

    #[test]
    fn a_file_rule_matches_the_path_and_not_the_content() {
        let rules = AlwaysAllow::default();
        let first = candidate(
            "Edit",
            &json!({ "file_path": "src/a.rs", "old_string": "x", "new_string": "y" }),
        )
        .expect("candidate");
        rules.add("t1", &first);
        let other_content = candidate(
            "Edit",
            &json!({ "file_path": "src/a.rs", "old_string": "p", "new_string": "q" }),
        )
        .expect("candidate");
        let other_path = candidate(
            "Edit",
            &json!({ "file_path": "src/b.rs", "old_string": "x", "new_string": "y" }),
        )
        .expect("candidate");
        assert!(rules.allows("t1", &other_content));
        assert!(!rules.allows("t1", &other_path));
        let write = candidate("Write", &json!({ "file_path": "src/a.rs", "content": "" }))
            .expect("candidate");
        assert!(!rules.allows("t1", &write), "another tool is another rule");
    }

    #[test]
    fn a_revoked_rule_asks_again() {
        let rules = AlwaysAllow::default();
        let rule = rules.add("t1", &bash("ls")).expect("rule");
        assert_eq!(rules.list("t1"), vec![rule.clone()]);
        assert!(rules.revoke("t1", rule.id));
        assert!(!rules.allows("t1", &bash("ls")));
        assert!(!rules.revoke("t1", rule.id));
        assert!(!rules.revoke("t2", rule.id));
    }

    #[test]
    fn requests_that_could_widen_a_rule_are_not_rememberable() {
        assert!(candidate(
            "Bash",
            &json!({ "command": "ls", "dangerouslyDisableSandbox": true })
        )
        .is_none());
        assert!(candidate("Bash", &json!({ "command": "" })).is_none());
        assert!(candidate("WebFetch", &json!({ "url": "https://example.com" })).is_none());
        assert!(candidate("Edit", &json!({ "old_string": "x" })).is_none());
        assert!(candidate("Bash", &json!(["ls"])).is_none());
    }

    #[test]
    fn the_task_is_the_part_of_the_key_before_the_tab() {
        assert_eq!(task_of("task1:agent"), "task1");
        assert_eq!(task_of("task1"), "task1");
    }

    #[test]
    fn a_prefix_rule_covers_later_commands_that_start_the_same_way() {
        let rules = AlwaysAllow::default();
        let first = bash("gh pr merge 481 --merge");
        assert_eq!(first.prefix_label().as_deref(), Some("gh pr merge"));
        let rule = rules.add_prefix("t1", &first).expect("prefix rule");
        assert_eq!(rule.kind, RuleKind::Prefix);
        assert_eq!(rule.target, "gh pr merge");
        assert!(
            !rules.allows("t1", &bash("gh pr merge 483 --merge 2>&1")),
            "a redirect is not simple"
        );
        assert!(rules.allows("t1", &bash("gh pr merge 483 --merge")));
        assert!(rules.allows("t1", &bash("gh pr merge 9")));
        assert!(!rules.allows("t1", &bash("gh pr view 9")));
        assert!(!rules.allows("t1", &bash("gh pr mergex 9")));
        assert!(
            !rules.allows("t2", &bash("gh pr merge 9")),
            "another task asks again"
        );
    }

    #[test]
    fn a_prefix_rule_never_covers_a_chained_or_substituted_command() {
        let rules = AlwaysAllow::default();
        rules
            .add_prefix("t1", &bash("gh pr merge 1"))
            .expect("prefix rule");
        for command in [
            "gh pr merge 1; rm -rf ~",
            "gh pr merge 1 && rm -rf ~",
            "gh pr merge 1 | sh",
            "gh pr merge $(rm -rf ~)",
            "gh pr merge `rm -rf ~`",
            "gh pr merge 1 > /etc/x",
            "FOO=1 gh pr merge 1",
            "gh pr merge 1\nrm x",
        ] {
            assert!(!rules.allows("t1", &bash(command)), "{command:?} must ask");
        }
    }

    #[test]
    fn a_prefix_is_offered_only_where_the_words_allow_it_and_never_for_edits() {
        assert!(AlwaysAllow::default()
            .add_prefix("t1", &bash("rm -rf x"))
            .is_none());
        assert!(AlwaysAllow::default()
            .add_prefix("t1", &bash("echo hi; rm x"))
            .is_none());
        let edit = candidate(
            "Edit",
            &json!({ "file_path": "src/a.rs", "old_string": "x", "new_string": "y" }),
        )
        .expect("candidate");
        assert!(edit.prefix_label().is_none());
        assert!(AlwaysAllow::default().add_prefix("t1", &edit).is_none());
    }

    #[test]
    fn an_exact_rule_and_a_prefix_rule_for_the_same_words_are_two_rules() {
        let rules = AlwaysAllow::default();
        let command = bash("cargo test");
        rules.add("t1", &command).expect("exact");
        rules.add_prefix("t1", &command).expect("prefix");
        rules.add_prefix("t1", &command).expect("again");
        assert_eq!(rules.list("t1").len(), 2);
    }
}
