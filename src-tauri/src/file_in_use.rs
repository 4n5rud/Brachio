//! Recognizes the failure Windows gives when a program still holds a file that is being deleted.

const MARKER: &str = "file in use: ";

/// If git's failure text says a file was in use (sharing violation 32, lock violation 33, "being
/// used by another process"), returns `file in use: <path>` for the interface. The path is the
/// first quoted name in the text, or empty when git named none.
#[must_use]
pub fn describe(message: &str) -> Option<String> {
    let lower = message.to_lowercase();
    let busy = lower.contains("used by another process")
        || lower.contains("os error 32")
        || lower.contains("os error 33")
        || lower.contains("device or resource busy")
        || ((lower.contains("failed to delete") || lower.contains("unlink of file"))
            && lower.contains("permission denied"));
    if !busy {
        return None;
    }
    let path = message
        .split('\'')
        .nth(1)
        .map(|quoted| quoted.replace('/', "\\"))
        .unwrap_or_default();
    Some(format!("{MARKER}{path}"))
}

#[cfg(test)]
mod tests {
    use super::describe;

    #[test]
    fn a_held_file_is_named_and_other_failures_are_left_alone() {
        assert_eq!(
            describe("error: failed to delete 'C:/repo.worktrees/t1/node_modules/.bin/vite.exe': Permission denied")
                .as_deref(),
            Some(r"file in use: C:\repo.worktrees\t1\node_modules\.bin\vite.exe")
        );
        assert_eq!(
            describe("The process cannot access the file because it is being used by another process. (os error 32)")
                .as_deref(),
            Some("file in use: ")
        );
        assert_eq!(describe("fatal: 'x' is not a working tree"), None);
    }
}
