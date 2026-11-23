//! Secret storage that fails closed.
//!
//! Brachio stores no secret today. Code that starts to must go through [`SecretStore`], so a
//! machine without OS-level encryption never gets a plaintext secret on disk.

use std::io;
use std::path::Path;

use crate::atomic_file::write_atomic;

/// Encryption provided by the operating system, such as DPAPI, Keychain or Secret Service.
pub trait SecretCipher {
    /// Whether the OS can encrypt for this user right now.
    fn is_available(&self) -> bool;

    /// Encrypts `plaintext` for the current user.
    ///
    /// # Errors
    ///
    /// Fails when the OS refuses.
    fn seal(&self, plaintext: &[u8]) -> io::Result<Vec<u8>>;

    /// Decrypts bytes produced by [`SecretCipher::seal`].
    ///
    /// # Errors
    ///
    /// Fails when the bytes are damaged or belong to another user.
    fn open(&self, sealed: &[u8]) -> io::Result<Vec<u8>>;
}

/// Why a secret was not stored or read.
#[derive(Debug, thiserror::Error)]
pub enum SecretStoreError {
    /// The OS cannot encrypt. Nothing was written or returned.
    #[error("OS-level encryption is unavailable, so the secret is not stored")]
    EncryptionUnavailable,
    /// The cipher or the disk failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Writes and reads secrets only in encrypted form.
#[derive(Debug)]
pub struct SecretStore<C> {
    cipher: C,
}

impl<C: SecretCipher> SecretStore<C> {
    /// Wraps the OS cipher.
    pub fn new(cipher: C) -> Self {
        Self { cipher }
    }

    /// Encrypts `secret` and writes it atomically to `path`.
    ///
    /// # Errors
    ///
    /// [`SecretStoreError::EncryptionUnavailable`] when the OS cannot encrypt. Nothing is written.
    pub fn save(&self, path: &Path, secret: &[u8]) -> Result<(), SecretStoreError> {
        if !self.cipher.is_available() {
            return Err(SecretStoreError::EncryptionUnavailable);
        }
        let sealed = self.cipher.seal(secret)?;
        write_atomic(path, &sealed)?;
        Ok(())
    }

    /// Reads and decrypts the secret at `path`.
    ///
    /// # Errors
    ///
    /// [`SecretStoreError::EncryptionUnavailable`] when the OS cannot decrypt.
    pub fn load(&self, path: &Path) -> Result<Vec<u8>, SecretStoreError> {
        if !self.cipher.is_available() {
            return Err(SecretStoreError::EncryptionUnavailable);
        }
        Ok(self.cipher.open(&std::fs::read(path)?)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct XorCipher {
        available: bool,
    }

    impl SecretCipher for XorCipher {
        fn is_available(&self) -> bool {
            self.available
        }
        fn seal(&self, plaintext: &[u8]) -> io::Result<Vec<u8>> {
            Ok(plaintext.iter().map(|byte| byte ^ 0x5a).collect())
        }
        fn open(&self, sealed: &[u8]) -> io::Result<Vec<u8>> {
            self.seal(sealed)
        }
    }

    #[test]
    fn stores_only_the_encrypted_form() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("token");
        let store = SecretStore::new(XorCipher { available: true });
        store.save(&path, b"hunter2").expect("save");
        assert_ne!(std::fs::read(&path).expect("raw"), b"hunter2");
        assert_eq!(store.load(&path).expect("load"), b"hunter2");
    }

    #[test]
    fn without_os_encryption_nothing_is_written_or_read() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("token");
        let store = SecretStore::new(XorCipher { available: false });
        assert!(matches!(
            store.save(&path, b"hunter2"),
            Err(SecretStoreError::EncryptionUnavailable)
        ));
        assert!(!path.exists());
        std::fs::write(&path, b"leftover").expect("write");
        assert!(matches!(
            store.load(&path),
            Err(SecretStoreError::EncryptionUnavailable)
        ));
    }
}
