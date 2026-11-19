//! Branch and folder names derived from task titles.

const MAX_SLUG_CHARS: usize = 40;
const MAX_SLUG_BYTES: usize = 80;
const BRANCH_PREFIX: &str = "brachio/";

/// Turns a task title into a branch-safe slug: lowercase letters and digits of any script
/// (git accepts them in branch names) joined by single hyphens, so a Korean title stays readable.
/// A title without any letter or digit falls back to "task"; the short id carries uniqueness.
#[must_use]
pub fn slugify(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    for character in title.chars() {
        if character.is_alphanumeric() {
            slug.extend(character.to_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
        if slug.chars().count() >= MAX_SLUG_CHARS || slug.len() >= MAX_SLUG_BYTES {
            break;
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "task".to_owned()
    } else {
        slug.to_owned()
    }
}

/// `brachio/<slug>-<short id>`.
#[must_use]
pub fn branch_name(title: &str, short_id: &str) -> String {
    format!("{BRANCH_PREFIX}{}-{short_id}", slugify(title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_keeps_ascii_words() {
        assert_eq!(
            slugify("Fix Safari export button!"),
            "fix-safari-export-button"
        );
    }

    #[test]
    fn slug_keeps_korean_words_apart_for_different_titles() {
        assert_eq!(slugify("로그인 버그 수정"), "로그인-버그-수정");
        assert_eq!(slugify("결제 오류"), "결제-오류");
        assert_eq!(slugify("로그인 bug 수정"), "로그인-bug-수정");
    }

    #[test]
    fn slug_of_a_title_without_letters_falls_back() {
        assert_eq!(slugify("!!! ???"), "task");
    }

    #[test]
    fn slug_is_capped() {
        let long = "a".repeat(100);
        assert_eq!(slugify(&long).len(), MAX_SLUG_CHARS);
    }

    #[test]
    fn branch_has_prefix_and_id() {
        assert_eq!(branch_name("Add QA", "k3f9"), "brachio/add-qa-k3f9");
    }
}
