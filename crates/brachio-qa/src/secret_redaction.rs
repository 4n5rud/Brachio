//! Removes common credentials from QA output before it is stored or shown.

use std::sync::LazyLock;

use regex::Regex;

const REDACTED: &str = "[redacted]";

/// Each pattern keeps the part before `${keep}` and replaces the rest.
static PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    let rules: [(&str, &str); 8] = [
        (
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z ]*PRIVATE KEY-----|\z)",
            REDACTED,
        ),
        (
            r"([A-Za-z][A-Za-z0-9+.\-]*://)[^/\s:@]+:[^/\s@]+@",
            "${1}[redacted]@",
        ),
        (r"gh[pousr]_[A-Za-z0-9]{20,}", REDACTED),
        (r"github_pat_[A-Za-z0-9_]{20,}", REDACTED),
        (r"sk-[A-Za-z0-9_\-]{16,}", REDACTED),
        (r"xox[abprs]-[A-Za-z0-9\-]{10,}", REDACTED),
        (r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b", REDACTED),
        (
            r"(?i)\b(bearer)\s+[A-Za-z0-9._~+/=\-]{8,}",
            "${1} [redacted]",
        ),
    ];
    rules
        .into_iter()
        .filter_map(|(pattern, with)| Regex::new(pattern).ok().map(|regex| (regex, with)))
        .collect()
});

static ASSIGNMENT: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(password|passwd|pwd|token|secret|api[_-]?key|access[_-]?key)=([^&\s"']+)"#)
        .ok()
});

/// A cut-off output can end inside a token, too short for the full patterns to match.
static CUT_TOKEN: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:gh[pousr]_|github_pat_|sk-|xox[abprs]-|AKIA|ASIA|bearer )[A-Za-z0-9_.~+/=\-]*\z",
    )
    .ok()
});

/// Replaces credentials in `text` with `[redacted]`: private key blocks, tokens with known
/// prefixes (`ghp_`, `github_pat_`, `sk-`, `xox[abprs]-`), AWS access key ids, `Bearer` tokens,
/// passwords inside URLs, and `password=`, `token=` style values. Binary output is already
/// lossy text here. `cut` says the text was truncated, so a token cut short at its end goes too.
#[must_use]
pub fn redact_secrets(text: &str, cut: bool) -> String {
    let mut text = text.to_owned();
    for (regex, with) in PATTERNS.iter() {
        text = regex.replace_all(&text, *with).into_owned();
    }
    if let Some(regex) = ASSIGNMENT.as_ref() {
        text = regex
            .replace_all(&text, format!("${{1}}={REDACTED}").as_str())
            .into_owned();
    }
    if cut {
        if let Some(regex) = CUT_TOKEN.as_ref() {
            text = regex.replace(&text, REDACTED).into_owned();
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::redact_secrets;

    fn gone(input: &str, secret: &str) {
        let output = redact_secrets(input, false);
        assert!(!output.contains(secret), "{output}");
        assert!(output.contains("[redacted]"), "{output}");
    }

    #[test]
    fn github_tokens_are_redacted() {
        gone(
            "token ghp_abcdefghijklmnopqrstuvwxyz0123456789 end",
            "ghp_abcdef",
        );
        gone(
            "github_pat_11ABCDEFG0abcdefghijkl_mnopqrstuvwxyz0123",
            "github_pat_11",
        );
    }

    #[test]
    fn api_keys_are_redacted() {
        gone("key=x sk-abcdefghijklmnopqrstuvwx", "sk-abcdef");
        gone("slack xoxb-1234567890-abcdefghij", "xoxb-1234");
        gone("aws AKIAIOSFODNN7EXAMPLE here", "AKIAIOSFODNN7EXAMPLE");
    }

    #[test]
    fn bearer_values_are_redacted_but_the_scheme_stays() {
        let output = redact_secrets("Authorization: Bearer abc.def-123_456", false);
        assert_eq!(output, "Authorization: Bearer [redacted]");
    }

    #[test]
    fn url_credentials_are_redacted_but_the_host_stays() {
        let output = redact_secrets("clone https://user:hunter2@example.com/repo.git", false);
        assert_eq!(output, "clone https://[redacted]@example.com/repo.git");
    }

    #[test]
    fn assignment_values_are_redacted() {
        let output = redact_secrets("GET /x?token=abc123&page=2 password=hunter2", false);
        assert_eq!(output, "GET /x?token=[redacted]&page=2 password=[redacted]");
    }

    #[test]
    fn a_private_key_block_is_redacted_across_lines() {
        let input =
            "a\n-----BEGIN RSA PRIVATE KEY-----\nMIIabc\ndef\n-----END RSA PRIVATE KEY-----\nb";
        assert_eq!(redact_secrets(input, false), "a\n[redacted]\nb");
    }

    #[test]
    fn a_key_cut_off_by_truncation_is_redacted_to_the_end() {
        assert_eq!(
            redact_secrets("x\n-----BEGIN PRIVATE KEY-----\nMIIabc", true),
            "x\n[redacted]"
        );
    }

    #[test]
    fn a_token_cut_short_at_the_end_is_redacted_only_when_truncated() {
        assert_eq!(redact_secrets("tok ghp_abc", true), "tok [redacted]");
        assert_eq!(redact_secrets("tok ghp_abc", false), "tok ghp_abc");
    }

    #[test]
    fn ordinary_output_is_untouched() {
        let text = "test result: ok. 3 passed; see https://example.com/a?b=1";
        assert_eq!(redact_secrets(text, true), text);
    }
}
