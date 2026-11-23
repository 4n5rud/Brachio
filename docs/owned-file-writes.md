# Files Brachio owns

Rules for every file the app, the runtime or the helper writes outside SQLite: session journal
records, pending launch records, generated hook settings and plugin files, and small state files.
The code that implements them is the `brachio-fs` crate.

## Writing

Replace a file in one step with `brachio_fs::write_atomic`:

1. Write a scratch file in the same directory (`.<name>.<pid>-<n>.tmp`).
2. Flush it to disk (`sync_all`).
3. Rename it over the target. `std::fs::rename` replaces an existing file on Windows too; the
   helper tries again briefly when another process holds the target open.
4. On Unix, flush the directory so the rename survives a power loss.

A crash or a concurrent reader sees the old file or the new one, never half of either. Do not use
`fs::write` on a file that is read back later. Scratch files with the `.tmp` extension are ignored
by readers and may be left behind by a crash.

Log files and files that belong to the user (task files, repository files) are not covered: logs
are append-only, and user files follow the user's own tools.

## Reading

Read an owned file with `brachio_fs::read_or_quarantine`. When the content does not parse:

1. The file is renamed to `<name>.corrupt-<unix ms>`. It is never deleted and never overwritten.
2. The caller falls back to defaults, and logs a warning with the new path.

A corrupt file must not crash startup and must not be silently replaced by the next write.

One exception: a record that guards a side effect that may already have happened (the pending
terminal launch records) is not reset to defaults. A damaged one reports the launch as uncertain,
so the app never starts a second shell by mistake.

## Secrets

Brachio stores no secret on disk today. If a feature needs to, it must use
`brachio_fs::SecretStore` with the operating system's encryption (DPAPI, Keychain, Secret Service).
The store fails closed: when the OS cannot encrypt, saving returns `EncryptionUnavailable`, nothing
is written, and the feature stays off or asks again each time. A plaintext file, an obfuscated file
and a key kept next to the data are not acceptable fallbacks. Secrets also stay out of logs, error
messages and command-line arguments.
