//! Preparing a new worktree from the repository's own `brachio.toml`.
//!
//! ```toml
//! [setup]
//! copy = [".env"]            # files copied from the repository into every new worktree
//! run = ["pnpm install"]     # commands the user is asked to run in the new worktree
//! ```

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

/// The file a repository uses to say how its worktrees are prepared.
pub const CONFIG_FILE: &str = "brachio.toml";

/// Why a worktree could not be prepared.
#[derive(Debug, thiserror::Error)]
#[must_use]
pub enum SetupError {
    /// The file exists but could not be read or copied.
    #[error("{path}: {source}")]
    Io {
        /// The file involved.
        path: PathBuf,
        /// The operating system's reason.
        source: io::Error,
    },
    /// `brachio.toml` is not valid.
    #[error("{path}: {message}")]
    Invalid {
        /// The configuration file.
        path: PathBuf,
        /// What is wrong with it.
        message: String,
    },
    /// A `copy` entry points outside the repository.
    #[error("{0}: copy entries must be plain paths inside the repository")]
    UnsafePath(String),
    /// A `run` entry is empty or holds a control character.
    #[error("{0:?}: run entries must be one line of printable text")]
    UnsafeCommand(String),
}

/// What to do for every new worktree of a repository.
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
pub struct SetupConfig {
    /// Files copied from the repository root, such as `.env`. A missing file is skipped.
    #[serde(default)]
    pub copy: Vec<String>,
    /// Commands for the user to run in the new worktree, one per entry.
    #[serde(default)]
    pub run: Vec<String>,
}

#[derive(Default, Deserialize)]
struct RepoConfig {
    #[serde(default)]
    setup: SetupConfig,
}

/// Reads the `[setup]` table of `<repo_root>/brachio.toml`. No file means nothing to do.
///
/// # Errors
/// Fails when the file exists but cannot be read, is not valid TOML, or lists a `copy` entry that
/// is absolute or climbs out of the repository.
pub fn read_setup(repo_root: &Path) -> Result<SetupConfig, SetupError> {
    let path = repo_root.join(CONFIG_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(SetupConfig::default()),
        Err(source) => return Err(SetupError::Io { path, source }),
    };
    let config: RepoConfig = toml::from_str(&text).map_err(|error| SetupError::Invalid {
        path: path.clone(),
        message: error.message().to_owned(),
    })?;
    if let Some(entry) = config
        .setup
        .copy
        .iter()
        .find(|entry| !is_plain_relative(entry))
    {
        return Err(SetupError::UnsafePath(entry.clone()));
    }
    // A command is typed into the user's shell after one confirmation. A line break would run a
    // second command, and an escape sequence can make the confirmation show something else.
    if let Some(command) = config
        .setup
        .run
        .iter()
        .find(|command| command.trim().is_empty() || command.chars().any(char::is_control))
    {
        return Err(SetupError::UnsafeCommand(command.clone()));
    }
    Ok(config.setup)
}

/// Copies the files named in `config.copy` from the repository into the worktree and returns
/// the entries that were copied. Entries that do not exist are skipped.
///
/// # Errors
/// Fails on an entry that is absolute or climbs out of the repository, which includes a path
/// that leaves the repository through a link, or when a copy fails.
pub fn copy_setup_files(
    repo_root: &Path,
    worktree: &Path,
    config: &SetupConfig,
) -> Result<Vec<String>, SetupError> {
    copy_files(repo_root, worktree, config)
}

/// Retries setup file copy while preserving files already in the worktree.
///
/// # Errors
/// Fails on an unsafe path or when a missing file cannot be copied.
pub fn retry_setup_files(
    repo_root: &Path,
    worktree: &Path,
    config: &SetupConfig,
) -> Result<Vec<String>, SetupError> {
    copy_files(repo_root, worktree, config)
}

fn copy_files(
    repo_root: &Path,
    worktree: &Path,
    config: &SetupConfig,
) -> Result<Vec<String>, SetupError> {
    let root = fs::canonicalize(repo_root).map_err(|source| SetupError::Io {
        path: repo_root.to_path_buf(),
        source,
    })?;
    let worktree = fs::canonicalize(worktree).map_err(|source| SetupError::Io {
        path: worktree.to_path_buf(),
        source,
    })?;
    let mut copied = Vec::new();
    for entry in &config.copy {
        if !is_plain_relative(entry) {
            return Err(SetupError::UnsafePath(entry.clone()));
        }
        let source = root.join(entry);
        let Ok(resolved) = fs::canonicalize(&source) else {
            continue;
        };
        if !resolved.starts_with(&root) {
            return Err(SetupError::UnsafePath(entry.clone()));
        }
        if !resolved.is_file() {
            continue;
        }
        let target = worktree.join(entry);
        let io_error = |path: &Path| {
            let path = path.to_path_buf();
            move |source| SetupError::Io { path, source }
        };
        if let Some(parent) = target.parent() {
            let ancestors = parent
                .strip_prefix(&worktree)
                .map_err(|_| SetupError::UnsafePath(entry.clone()))?;
            let mut directory = worktree.clone();
            for part in ancestors.components() {
                directory.push(part);
                match fs::create_dir(&directory) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(io_error(&directory)(error)),
                }
                let actual = directory.canonicalize().map_err(io_error(&directory))?;
                if !actual.starts_with(&worktree) || !actual.is_dir() {
                    return Err(SetupError::UnsafePath(entry.clone()));
                }
            }
        }
        let mut input = fs::File::open(&resolved).map_err(io_error(&resolved))?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = match options.open(&target) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let info = fs::symlink_metadata(&target).map_err(io_error(&target))?;
                if info.is_file() && !info.file_type().is_symlink() {
                    continue;
                }
                return Err(SetupError::UnsafePath(entry.clone()));
            }
            Err(error) => return Err(io_error(&target)(error)),
        };
        if let Err(error) = std::io::copy(&mut input, &mut output) {
            drop(output);
            let _ = fs::remove_file(&target);
            return Err(io_error(&target)(error));
        }
        let permissions = input.metadata().map_err(io_error(&resolved))?.permissions();
        output
            .set_permissions(permissions)
            .map_err(io_error(&target))?;
        copied.push(entry.clone());
    }
    Ok(copied)
}

fn is_plain_relative(entry: &str) -> bool {
    !(entry.is_empty() || cfg!(windows) && entry.contains(':'))
        && Path::new(entry).components().all(|component| {
            matches!(component, Component::Normal(name)
                if !name.eq_ignore_ascii_case(".git"))
        })
}

#[cfg(test)]
mod tests {
    use super::{copy_setup_files, read_setup, retry_setup_files, SetupConfig, SetupError};
    use std::fs;

    #[test]
    fn a_repository_without_a_config_file_has_nothing_to_set_up() {
        let repo = tempfile::tempdir().expect("temp repo");
        assert_eq!(
            read_setup(repo.path()).expect("read"),
            SetupConfig::default()
        );
    }

    #[test]
    fn initial_setup_preserves_existing_files_and_copies_missing_ones() {
        let repo = tempfile::tempdir().expect("repo");
        let worktree = tempfile::tempdir().expect("worktree");
        fs::write(repo.path().join(".env"), "SOURCE").expect("source");
        fs::write(repo.path().join(".env.extra"), "EXTRA").expect("extra");
        fs::write(worktree.path().join(".env"), "USER").expect("existing");
        let config = SetupConfig {
            copy: vec![".env".into(), ".env.extra".into()],
            run: Vec::new(),
        };
        let copied = copy_setup_files(repo.path(), worktree.path(), &config).expect("setup");
        assert_eq!(copied, [".env.extra"]);
        assert_eq!(fs::read_to_string(worktree.path().join(".env")).expect("read"), "USER");
        assert_eq!(fs::read_to_string(worktree.path().join(".env.extra")).expect("read"), "EXTRA");
        assert!(copy_setup_files(repo.path(), worktree.path(), &config)
            .expect("idempotent setup")
            .is_empty());
    }

    #[test]
    fn retry_copies_missing_files_without_overwriting_user_changes() {
        let repo = tempfile::tempdir().expect("repo");
        let worktree = tempfile::tempdir().expect("worktree");
        fs::write(repo.path().join(".env"), "ORIGINAL").expect("write source");
        fs::write(repo.path().join(".env.extra"), "EXTRA").expect("write missing source");
        fs::write(worktree.path().join(".env"), "EDITED").expect("write user file");
        let config = SetupConfig {
            copy: vec![".env".into(), ".env.extra".into()],
            run: Vec::new(),
        };
        let copied = retry_setup_files(repo.path(), worktree.path(), &config).expect("retry");
        assert_eq!(copied, [".env.extra"]);
        assert_eq!(
            fs::read_to_string(worktree.path().join(".env")).expect("read user file"),
            "EDITED"
        );
    }

    #[test]
    fn a_run_command_with_a_line_break_or_escape_sequence_is_refused() {
        for bad in [
            r"pnpm install\nrm -rf x",
            r"echo \u001b[2Jhidden",
            r"echo a\rb",
            " ",
        ] {
            let folder = tempfile::tempdir().expect("temp dir");
            std::fs::write(
                folder.path().join(super::CONFIG_FILE),
                format!("[setup]\nrun = [\"{bad}\"]\n"),
            )
            .expect("write config");
            let error = read_setup(folder.path()).expect_err(bad);
            assert!(
                matches!(error, SetupError::UnsafeCommand(_)),
                "{bad}: {error}"
            );
        }
    }

    #[test]
    fn the_setup_table_lists_files_to_copy_and_commands_to_run() {
        let repo = tempfile::tempdir().expect("temp repo");
        fs::write(
            repo.path().join("brachio.toml"),
            "[setup]\ncopy = [\".env\", \"ui/.env.local\"]\nrun = [\"pnpm install\"]\n",
        )
        .expect("write config");
        let config = read_setup(repo.path()).expect("read");
        assert_eq!(config.copy, [".env", "ui/.env.local"]);
        assert_eq!(config.run, ["pnpm install"]);
    }

    #[test]
    fn a_broken_config_names_the_file_in_its_error() {
        let repo = tempfile::tempdir().expect("temp repo");
        fs::write(repo.path().join("brachio.toml"), "[setup\ncopy = 1").expect("write config");
        let error = read_setup(repo.path()).expect_err("invalid toml");
        assert!(matches!(error, SetupError::Invalid { .. }), "{error}");
        assert!(error.to_string().contains("brachio.toml"), "{error}");
    }

    #[test]
    fn copies_existing_files_skips_missing_ones_and_keeps_folders() {
        let repo = tempfile::tempdir().expect("temp repo");
        let worktree = tempfile::tempdir().expect("temp worktree");
        fs::write(repo.path().join(".env"), "TOKEN=placeholder\n").expect("write env");
        fs::create_dir(repo.path().join("ui")).expect("make ui");
        fs::write(repo.path().join("ui/.env.local"), "A=1\n").expect("write nested env");
        let config = SetupConfig {
            copy: vec![".env".into(), "ui/.env.local".into(), ".env.missing".into()],
            run: Vec::new(),
        };
        let copied = copy_setup_files(repo.path(), worktree.path(), &config).expect("copy");
        assert_eq!(copied, [".env", "ui/.env.local"]);
        assert_eq!(
            fs::read_to_string(worktree.path().join("ui/.env.local")).expect("read copy"),
            "A=1\n"
        );
    }

    #[test]
    fn a_config_that_copies_from_outside_the_repository_is_refused_when_read() {
        let repo = tempfile::tempdir().expect("temp repo");
        fs::write(
            repo.path().join("brachio.toml"),
            "[setup]\ncopy = [\"../secret.txt\"]\n",
        )
        .expect("write config");
        let error = read_setup(repo.path()).expect_err("unsafe entry");
        assert!(matches!(error, SetupError::UnsafePath(_)), "{error}");
    }

    #[test]
    fn entries_that_leave_the_repository_are_refused() {
        let repo = tempfile::tempdir().expect("temp repo");
        let worktree = tempfile::tempdir().expect("temp worktree");
        let outside = repo.path().join("..").join("secret.txt");
        // `C:\x` is only a drive-qualified path on Windows; elsewhere it is an odd file name.
        let mut unsafe_entries = vec!["../secret.txt", "/etc/passwd", ""];
        if cfg!(windows) {
            unsafe_entries.push("C:\\Windows\\win.ini");
        }
        for entry in unsafe_entries {
            let config = SetupConfig {
                copy: vec![entry.to_owned()],
                run: Vec::new(),
            };
            let error =
                copy_setup_files(repo.path(), worktree.path(), &config).expect_err("unsafe entry");
            assert!(
                matches!(error, SetupError::UnsafePath(_)),
                "{entry}: {error}"
            );
        }
        assert!(!outside.exists());
    }
}
