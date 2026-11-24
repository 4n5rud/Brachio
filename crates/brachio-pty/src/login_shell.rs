//! Shells that Finder or the Dock starts inherit launchd's minimal environment, so a shell tab
//! must read the user's profile files the way Terminal.app does: as a login shell.

use std::path::Path;

/// Arguments that make `program` a login shell on this platform, none elsewhere.
#[must_use]
pub fn login_shell_arguments(program: &Path) -> Vec<String> {
    login_arguments(program, cfg!(target_os = "macos"))
}

fn login_arguments(program: &Path, wanted: bool) -> Vec<String> {
    let name = program
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if wanted && matches!(name, "zsh" | "bash" | "sh" | "ksh" | "dash" | "fish") {
        vec!["-l".to_owned()]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::login_arguments;

    #[test]
    fn posix_shells_become_login_shells_only_where_wanted() {
        assert_eq!(login_arguments(Path::new("/bin/zsh"), true), ["-l"]);
        assert_eq!(
            login_arguments(Path::new("/opt/homebrew/bin/fish"), true),
            ["-l"]
        );
        assert!(login_arguments(Path::new("/bin/zsh"), false).is_empty());
        assert!(login_arguments(Path::new("/usr/local/bin/nu"), true).is_empty());
        assert!(login_arguments(Path::new("cmd.exe"), true).is_empty());
    }
}
