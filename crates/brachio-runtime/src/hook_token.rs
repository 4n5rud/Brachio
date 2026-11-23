//! A secret that each app launch shares with the helper programs it starts, so the hook socket
//! can tell them from any other process that can reach it.
//!
//! The app writes a fresh token to a file in the user's private state folder when it starts.
//! Helpers read that file for every connection instead of keeping a copy in their environment,
//! so an agent that was started by an earlier launch, and still runs after the app was updated
//! or restarted, uses the new token on its next hook call.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::io::BufReader;

use crate::paths::{ensure_private_directory, state_directory};
use crate::transport;
use crate::wire::{read_message, write_message, MAX_MESSAGE};
use interprocess::local_socket::tokio::Stream;

/// The first message on a connection to a socket that requires a token.
#[derive(Serialize, Deserialize)]
struct Hello {
    hook_token: String,
}

fn token_path(endpoint: &str) -> io::Result<PathBuf> {
    let safe: String = endpoint
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    Ok(state_directory()?
        .join("hook-tokens")
        .join(format!("{safe}.token")))
}

/// Makes a new token for this launch and stores it where helpers look for it, replacing the
/// previous launch's token.
///
/// # Errors
/// Fails when the private folder or the file cannot be written.
pub fn issue(endpoint: &str) -> io::Result<String> {
    let path = token_path(endpoint)?;
    if let Some(folder) = path.parent() {
        ensure_private_directory(folder)?;
    }
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    brachio_fs::write_atomic(&path, token.as_bytes())?;
    Ok(token)
}

/// The token of the running app, or `None` when the app has not stored one.
#[must_use]
pub fn load(endpoint: &str) -> Option<String> {
    let text = fs::read_to_string(token_path(endpoint).ok()?).ok()?;
    let token = text.trim();
    (!token.is_empty()).then(|| token.to_owned())
}

/// Compares in time that does not depend on how many leading characters agree.
#[must_use]
pub fn matches(expected: &str, given: &str) -> bool {
    let (expected, given) = (expected.as_bytes(), given.as_bytes());
    let difference = expected.len() ^ given.len();
    let mut folded = u8::from(difference != 0);
    for (index, byte) in expected.iter().enumerate() {
        folded |= byte ^ given.get(index).copied().unwrap_or(0);
    }
    folded == 0
}

/// Whether `message` is a hello that carries `expected`.
#[must_use]
pub fn accepts(expected: &str, message: &[u8]) -> bool {
    serde_json::from_slice::<Hello>(message).is_ok_and(|hello| matches(expected, &hello.hook_token))
}

/// Connects to the hook socket and, when the app stored a token, sends it first. Without a token
/// nothing extra is sent, which is what an app that does not ask for one expects.
///
/// # Errors
/// Fails when no app accepts the connection.
pub async fn connect(endpoint: &str) -> io::Result<Stream> {
    let mut stream = transport::connect(endpoint).await?;
    if let Some(hook_token) = load(endpoint) {
        write_message(&mut stream, &Hello { hook_token }, MAX_MESSAGE)
            .await
            .map_err(io::Error::other)?;
    }
    Ok(stream)
}

/// Reads the hello the server asked for. `false` when it is missing or wrong.
pub async fn verify(reader: &mut BufReader<Stream>, expected: &str) -> bool {
    matches!(
        read_message(reader, MAX_MESSAGE).await,
        Ok(Some(message)) if accepts(expected, &message)
    )
}

#[cfg(test)]
mod tests {
    use super::{accepts, matches};

    #[test]
    fn only_the_exact_token_matches() {
        assert!(matches("abc", "abc"));
        assert!(!matches("abc", "abd"));
        assert!(!matches("abc", "ab"));
        assert!(!matches("abc", "abcd"));
        assert!(!matches("abc", ""));
    }

    #[test]
    fn a_hello_needs_the_field_and_the_value() {
        assert!(accepts("t0k", br#"{"hook_token":"t0k"}"#));
        assert!(!accepts("t0k", br#"{"hook_token":"other"}"#));
        assert!(!accepts("t0k", br#"{"key":"k","session_id":"s"}"#));
        assert!(!accepts("t0k", b"not json"));
    }
}
