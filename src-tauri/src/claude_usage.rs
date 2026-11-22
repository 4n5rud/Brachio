//! Read-only Claude Code plan usage for the status bar.
//!
//! The 5-hour and weekly percentages come from Anthropic's own usage endpoint, called with the
//! access token Claude Code already stored for this user. The token is read from the credentials
//! file (or, on macOS, the login Keychain entry Claude Code writes), handed to `curl` on stdin (never on the command line), and sent to that one HTTPS host.
//! It is not refreshed, logged or returned to the UI. Results are cached for a minute.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;
use tauri::State;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::timeout;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const CACHE_TTL: Duration = Duration::from_secs(60);
const FORCE_FLOOR: Duration = Duration::from_secs(5);
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);
const RESPONSE_LIMIT: u64 = 64 * 1024;
const CREDENTIALS_LIMIT: u64 = 256 * 1024;
#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";
#[cfg(target_os = "macos")]
const KEYCHAIN_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// One usage window as the UI draws it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    used_percent: f64,
    /// Unix seconds.
    resets_at: Option<i64>,
}

/// Plan usage for the signed-in Claude Code account.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClaudeUsage {
    plan: Option<String>,
    session: Option<UsageWindow>,
    weekly: Option<UsageWindow>,
}

/// The stored sign-in, reduced to what the request needs.
#[derive(Debug, PartialEq)]
struct Credentials {
    token: String,
    plan: Option<String>,
    expires_at_ms: Option<i64>,
}

/// Last answer, kept so the bar never asks more than once a minute.
#[derive(Default)]
pub struct ClaudeUsageCache(Mutex<Option<(Instant, Result<ClaudeUsage, String>)>>);

fn parse_credentials(text: &str) -> Option<Credentials> {
    let root: Value = serde_json::from_str(text).ok()?;
    let oauth = root.get("claudeAiOauth")?;
    let token = oauth.get("accessToken")?.as_str()?;
    let safe = !token.is_empty()
        && token.len() <= 4096
        && token
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'"' && byte != b'\\');
    if !safe {
        return None;
    }
    let kind = oauth
        .get("subscriptionType")
        .and_then(Value::as_str)
        .map(str::trim);
    let tier = oauth
        .get("rateLimitTier")
        .and_then(Value::as_str)
        .map(str::trim);
    Some(Credentials {
        token: token.to_owned(),
        plan: kind
            .filter(|kind| !kind.is_empty())
            .map(|kind| plan_name(kind, tier)),
        expires_at_ms: oauth.get("expiresAt").and_then(Value::as_i64),
    })
}

fn plan_name(kind: &str, tier: Option<&str>) -> String {
    let multiplier = tier
        .and_then(|tier| tier.rsplit_once("max_"))
        .map(|(_, rest)| rest)
        .filter(|rest| {
            rest.len() > 1
                && rest.len() <= 4
                && rest.ends_with('x')
                && rest[..rest.len() - 1]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit())
        });
    let mut chars = kind.chars();
    let name: String = chars
        .next()
        .map(|first| {
            first
                .to_uppercase()
                .chain(chars.flat_map(char::to_lowercase))
                .collect()
        })
        .unwrap_or_default();
    match multiplier {
        Some(multiplier) if kind.eq_ignore_ascii_case("max") => format!("{name} {multiplier}"),
        _ => name,
    }
}

/// Seconds since the epoch for `YYYY-MM-DDTHH:MM:SS[.fraction](Z|+HH:MM|-HH:MM)`.
fn parse_rfc3339(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20
        || !text.is_ascii()
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b'T' | b't' | b' ')
    {
        return None;
    }
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut rest = &text[19..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        rest = &fraction[digits..];
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            if rest.len() != 6 || rest.as_bytes()[3] != b':' {
                return None;
            }
            let hours = rest.get(1..3)?.parse::<i64>().ok()?;
            let minutes = rest.get(4..6)?.parse::<i64>().ok()?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            sign * (hours * 3600 + minutes * 60)
        }
    };
    // Days from the civil date (proleptic Gregorian), counted from 1970-01-01.
    let shifted_year = if month <= 2 { year - 1 } else { year };
    let era = shifted_year.div_euclid(400);
    let year_of_era = shifted_year.rem_euclid(400);
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

fn parse_window(value: Option<&Value>) -> Option<UsageWindow> {
    let value = value?;
    let percent = value.get("utilization").and_then(Value::as_f64)?;
    if !percent.is_finite() {
        return None;
    }
    Some(UsageWindow {
        used_percent: percent.clamp(0.0, 100.0),
        resets_at: value
            .get("resets_at")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339),
    })
}

fn parse_usage(body: &str, plan: Option<String>) -> Result<ClaudeUsage, String> {
    let root: Value =
        serde_json::from_str(body).map_err(|_| "Claude returned unreadable usage.".to_owned())?;
    let session = parse_window(root.get("five_hour"));
    let weekly = parse_window(root.get("seven_day"));
    if session.is_none() && weekly.is_none() {
        return Err("Claude returned no usage windows for this plan.".to_owned());
    }
    Ok(ClaudeUsage {
        plan,
        session,
        weekly,
    })
}

fn credentials_path() -> Option<PathBuf> {
    let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
            Some(PathBuf::from(home).join(".claude"))
        })?;
    Some(dir.join(".credentials.json"))
}

async fn read_credentials_file() -> Option<String> {
    let path = credentials_path()?;
    if !path.exists() {
        return None;
    }
    let file = tokio::fs::File::open(path).await.ok()?;
    let mut text = String::new();
    file.take(CREDENTIALS_LIMIT)
        .read_to_string(&mut text)
        .await
        .ok()?;
    Some(text)
}

/// Claude Code on macOS keeps its credentials JSON in the login Keychain; may show a permission prompt.
#[cfg(target_os = "macos")]
async fn read_keychain_credentials() -> Option<String> {
    let mut child = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let output = child.stdout.take()?;
    let mut bytes = Vec::new();
    let read = timeout(KEYCHAIN_TIMEOUT, async {
        output
            .take(CREDENTIALS_LIMIT)
            .read_to_end(&mut bytes)
            .await?;
        child.wait().await
    })
    .await;
    if !read.ok()?.ok()?.success() {
        return None;
    }
    String::from_utf8(bytes).ok()
}

async fn read_credentials() -> Result<Credentials, String> {
    let signed_out = || "Claude Code credentials were not found on this computer.".to_owned();
    let text = match read_credentials_file().await {
        Some(text) => Some(text),
        #[cfg(target_os = "macos")]
        None => read_keychain_credentials().await,
        #[cfg(not(target_os = "macos"))]
        None => None,
    }
    .ok_or_else(signed_out)?;
    let credentials = parse_credentials(&text).ok_or_else(signed_out)?;
    let now_ms = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis()),
    )
    .unwrap_or(0);
    if credentials
        .expires_at_ms
        .is_some_and(|expires| expires <= now_ms)
    {
        return Err(
            "The Claude Code sign-in has expired. Run Claude Code once to renew it.".to_owned(),
        );
    }
    Ok(credentials)
}

async fn fetch_body(token: &str) -> Result<String, String> {
    let failed = || "Claude usage could not be read.".to_owned();
    let mut command = Command::new("curl");
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command
        .args([
            "--silent",
            "--fail",
            "--proto",
            "=https",
            "--max-time",
            "10",
            "--max-filesize",
            "65536",
            "--config",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| failed())?;
    let mut input = child.stdin.take().ok_or_else(failed)?;
    let output = child.stdout.take().ok_or_else(failed)?;
    let config = format!(
        "url = \"{USAGE_URL}\"\nheader = \"Authorization: Bearer {token}\"\nheader = \"anthropic-beta: oauth-2025-04-20\"\n"
    );
    input
        .write_all(config.as_bytes())
        .await
        .map_err(|_| failed())?;
    drop(input);
    let mut body = Vec::new();
    output
        .take(RESPONSE_LIMIT)
        .read_to_end(&mut body)
        .await
        .map_err(|_| failed())?;
    let status = child.wait().await.map_err(|_| failed())?;
    if !status.success() {
        return Err(failed());
    }
    String::from_utf8(body).map_err(|_| failed())
}

async fn read_claude_usage() -> Result<ClaudeUsage, String> {
    let credentials = read_credentials().await?;
    let body = timeout(FETCH_TIMEOUT, fetch_body(&credentials.token))
        .await
        .map_err(|_| "Claude usage lookup timed out.".to_owned())??;
    parse_usage(&body, credentials.plan)
}

/// Reads the signed-in account's 5-hour and weekly usage; `force` is the manual refresh.
#[tauri::command]
pub async fn claude_usage(
    force: bool,
    cache: State<'_, ClaudeUsageCache>,
) -> Result<ClaudeUsage, String> {
    let mut last = cache.0.lock().await;
    if let Some((at, answer)) = last.as_ref() {
        let reuse = if force { FORCE_FLOOR } else { CACHE_TTL };
        if at.elapsed() < reuse {
            return answer.clone();
        }
    }
    let answer = read_claude_usage().await;
    *last = Some((Instant::now(), answer.clone()));
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREDENTIALS: &str = r#"{"claudeAiOauth":{"accessToken":"sample-token","refreshToken":"sample-refresh","expiresAt":4102444800000,"subscriptionType":"max","rateLimitTier":"default_claude_max_5x"},"other":{}}"#;
    const USAGE: &str = r#"{"five_hour":{"utilization":15.5,"resets_at":"2030-01-01T12:00:00.123456+00:00"},"seven_day":{"utilization":140,"resets_at":"2030-01-05T03:30:00Z"},"seven_day_opus":null}"#;

    #[test]
    fn credentials_yield_token_and_plan() {
        let credentials = parse_credentials(CREDENTIALS).expect("credentials");
        assert_eq!(credentials.token, "sample-token");
        assert_eq!(credentials.plan.as_deref(), Some("Max 5x"));
        assert_eq!(credentials.expires_at_ms, Some(4_102_444_800_000));
    }

    #[test]
    fn credentials_reject_missing_or_unsafe_tokens() {
        assert_eq!(parse_credentials("{}"), None);
        assert_eq!(parse_credentials("not json"), None);
        assert_eq!(
            parse_credentials(r#"{"claudeAiOauth":{"accessToken":"a\"b"}}"#),
            None
        );
        assert_eq!(
            parse_credentials(r#"{"claudeAiOauth":{"accessToken":"a\nb"}}"#),
            None
        );
    }

    #[test]
    fn plan_names_are_capitalised() {
        assert_eq!(plan_name("pro", Some("default_claude_ai")), "Pro");
        assert_eq!(plan_name("max", Some("default_claude_max_20x")), "Max 20x");
        assert_eq!(plan_name("team", None), "Team");
    }

    #[test]
    fn timestamps_convert_to_unix_seconds() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339("2030-01-01T12:00:00.123456+00:00"),
            Some(1_893_499_200)
        );
        assert_eq!(
            parse_rfc3339("2030-01-01T12:00:00+09:00"),
            Some(1_893_499_200 - 32_400)
        );
        assert_eq!(parse_rfc3339("2024-03-01T00:00:00Z"), Some(1_709_251_200));
        assert_eq!(parse_rfc3339("yesterday"), None);
        assert_eq!(parse_rfc3339("2030-13-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339("2030-01-01T12:00:00"), None);
    }

    #[test]
    fn usage_maps_both_windows_and_clamps_percent() {
        let usage = parse_usage(USAGE, Some("Pro".to_owned())).expect("usage");
        assert_eq!(usage.plan.as_deref(), Some("Pro"));
        assert_eq!(
            usage.session,
            Some(UsageWindow {
                used_percent: 15.5,
                resets_at: Some(1_893_499_200)
            })
        );
        assert_eq!(
            usage.weekly,
            Some(UsageWindow {
                used_percent: 100.0,
                resets_at: Some(1_893_814_200)
            })
        );
    }

    #[test]
    fn usage_without_windows_or_with_garbage_is_an_error() {
        assert!(parse_usage(r#"{"five_hour":null,"seven_day":null}"#, None).is_err());
        assert!(parse_usage("<html>", None).is_err());
        assert!(parse_usage(r#"{"five_hour":{"utilization":"high"}}"#, None).is_err());
    }

    #[test]
    fn a_single_window_is_enough() {
        let usage = parse_usage(r#"{"five_hour":{"utilization":3,"resets_at":"bad"}}"#, None)
            .expect("usage");
        assert_eq!(
            usage.session,
            Some(UsageWindow {
                used_percent: 3.0,
                resets_at: None
            })
        );
        assert_eq!(usage.weekly, None);
    }
}
