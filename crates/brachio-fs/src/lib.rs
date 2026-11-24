//! Files Brachio owns: crash-safe writes, corrupt-file quarantine, and secret storage that fails
//! closed. The rules are in `docs/owned-file-writes.md`.

mod atomic_file;
mod secret_storage;

pub use atomic_file::{
    quarantine_corrupt, read_or_quarantine, write_atomic, write_atomic_checked, Stored,
};
pub use secret_storage::{SecretCipher, SecretStore, SecretStoreError};
