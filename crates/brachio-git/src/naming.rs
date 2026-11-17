const MAX_SLUG_LEN: usize = 40;
const BRANCH_PREFIX: &str = "brachio/";

/// Turns a task title into a branch-safe slug: lowercase ASCII letters,
/// digits and single hyphens. Non-ASCII titles (Korean, for example) fall
/// back to "task" so the short id carries uniqueness.
#[must_use]
pub fn slugify(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
        if slug.len() >= MAX_SLUG_LEN {
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
    fn slug_of_korean_title_falls_back() {
        assert_eq!(slugify("로그인 버그 수정"), "task");
        assert_eq!(slugify("로그인 bug 수정"), "bug");
    }

    #[test]
    fn slug_is_capped() {
        let long = "a".repeat(100);
        assert_eq!(slugify(&long).len(), MAX_SLUG_LEN);
    }

    #[test]
    fn branch_has_prefix_and_id() {
        assert_eq!(branch_name("Add QA", "k3f9"), "brachio/add-qa-k3f9");
    }
}
