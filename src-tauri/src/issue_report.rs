//! Opens a pre-filled "new issue" page for the project, from a title and body the user reviewed.

use std::fmt::Write as _;

const NEW_ISSUE: &str = "https://github.com/4n5rud/Brachio/issues/new";
/// Browsers and GitHub start refusing addresses a little above 8 KB.
const MAX_URL_BYTES: usize = 7_500;
const MAX_TITLE_CHARS: usize = 200;
const TRUNCATED: &str = "\n\n… (truncated, attach brachio.log if useful)";

fn encode_into(out: &mut String, text: &str) {
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
}

fn encoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    encode_into(&mut out, text);
    out
}

/// The new-issue address on the project's own page; only the title and body come from the caller.
fn issue_url(title: &str, body: &str) -> String {
    let title: String = title.trim().chars().take(MAX_TITLE_CHARS).collect();
    let mut url = format!("{NEW_ISSUE}?labels=bug&title={}&body=", encoded(&title));
    let full = encoded(body);
    if url.len() + full.len() <= MAX_URL_BYTES {
        url.push_str(&full);
        return url;
    }
    let note = encoded(TRUNCATED);
    let room = MAX_URL_BYTES.saturating_sub(url.len() + note.len());
    let start = url.len();
    let mut piece = [0; 4];
    for character in body.chars() {
        let next = encoded(character.encode_utf8(&mut piece));
        if url.len() - start + next.len() > room {
            break;
        }
        url.push_str(&next);
    }
    url.push_str(&note);
    url
}

#[tauri::command]
pub async fn open_issue_draft(title: String, body: String) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("An issue needs a title.".to_owned());
    }
    crate::external_link::open_in_browser(issue_url(&title, &body)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_and_body_are_encoded_onto_the_project_new_issue_page() {
        let url = issue_url(" Crash & burn ", "a=b #1\n한");
        assert_eq!(
            url,
            "https://github.com/4n5rud/Brachio/issues/new?labels=bug\
             &title=Crash%20%26%20burn&body=a%3Db%20%231%0A%ED%95%9C"
        );
    }

    #[test]
    fn a_long_body_is_cut_on_a_character_boundary_and_marked() {
        let url = issue_url("t", &"한".repeat(5_000));
        assert!(url.len() <= MAX_URL_BYTES, "{}", url.len());
        assert!(url.starts_with("https://github.com/4n5rud/Brachio/issues/new?"));
        assert!(url.ends_with(&encoded(TRUNCATED)));
        let body = url.split("&body=").nth(1).expect("a body parameter");
        let kept = body
            .strip_suffix(&encoded(TRUNCATED))
            .expect("the truncation note");
        assert_eq!(kept.len() % "%ED%95%9C".len(), 0);
    }

    #[test]
    fn an_oversized_title_still_leaves_the_address_under_the_cap() {
        let url = issue_url(&"x".repeat(10_000), &"y".repeat(10_000));
        assert!(url.len() <= MAX_URL_BYTES, "{}", url.len());
    }
}
