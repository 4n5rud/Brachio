//! The local endpoint of a runtime: an owner-only named pipe on Windows, a private socket file elsewhere.

use std::io;

use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::{Listener, Stream};

/// Connect to the runtime that owns this endpoint.
///
/// # Errors
/// Fails when no runtime accepts the connection.
pub async fn connect(endpoint: &str) -> io::Result<Stream> {
    Stream::connect(name(endpoint)?).await
}

/// Start listening, failing when another runtime already holds the endpoint.
///
/// # Errors
/// Fails when the endpoint is taken or cannot be created.
pub fn listen(endpoint: &str) -> io::Result<Listener> {
    platform::listen(endpoint)
}

/// Remove the leftover of a runtime that has been shown to be dead.
///
/// # Errors
/// Fails when the leftover cannot be removed.
pub fn reclaim(endpoint: &str) -> io::Result<()> {
    platform::reclaim(endpoint)
}

#[cfg(windows)]
fn name(endpoint: &str) -> io::Result<interprocess::local_socket::Name<'static>> {
    use interprocess::local_socket::GenericNamespaced;
    endpoint.to_owned().to_ns_name::<GenericNamespaced>()
}

#[cfg(unix)]
fn name(endpoint: &str) -> io::Result<interprocess::local_socket::Name<'static>> {
    use interprocess::local_socket::GenericFilePath;
    platform::socket_path(endpoint)?.to_fs_name::<GenericFilePath>()
}

#[cfg(windows)]
mod platform {
    use std::io;

    use interprocess::local_socket::tokio::Listener;
    use interprocess::local_socket::ListenerOptions;
    use interprocess::os::windows::local_socket::ListenerOptionsExt;
    use interprocess::os::windows::security_descriptor::SecurityDescriptor;
    use widestring::U16CString;

    pub(super) fn listen(endpoint: &str) -> io::Result<Listener> {
        let descriptor = U16CString::from_str("D:P(A;;GA;;;OW)").map_err(io::Error::other)?;
        let descriptor = SecurityDescriptor::deserialize(&descriptor)?;
        ListenerOptions::new()
            .name(super::name(endpoint)?)
            .security_descriptor(descriptor)
            .create_tokio()
    }

    /// A pipe name vanishes with its process, so nothing is left to clean up.
    pub(super) fn reclaim(_endpoint: &str) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(unix)]
mod platform {
    use std::fs::{self, DirBuilder, Permissions};
    use std::io;
    use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
    use std::path::{Path, PathBuf};

    use interprocess::local_socket::tokio::Listener;
    use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};
    #[cfg(target_os = "linux")]
    use interprocess::os::unix::local_socket::ListenerOptionsExt;

    /// Unix socket paths are short (104 bytes on macOS), so the directory stays flat.
    const LONGEST_PATH: usize = 100;

    pub(super) fn socket_path(endpoint: &str) -> io::Result<PathBuf> {
        if endpoint.is_empty() || endpoint.contains(['/', '\\']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "endpoint must be a plain name",
            ));
        }
        let uid = rustix::process::geteuid().as_raw();
        let parent = trusted_temp_directory(&std::env::temp_dir(), uid)?;
        let directory = parent.join("brachio-runtime");
        ensure_private_directory(&directory, uid)?;
        let path = directory.join(format!("{endpoint}.sock"));
        if path.as_os_str().len() > LONGEST_PATH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "endpoint path is too long for a Unix socket",
            ));
        }
        Ok(path)
    }

    fn trusted_temp_directory(path: &Path, uid: u32) -> io::Result<PathBuf> {
        let resolved = fs::canonicalize(path)?;
        // Check from the root down. A validated parent may permit other users to create files
        // (such as sticky /tmp), but cannot let them replace one of our checked directories.
        let ancestors: Vec<_> = resolved.ancestors().collect();
        for ancestor in ancestors.into_iter().rev() {
            let metadata = fs::symlink_metadata(ancestor)?;
            let mode = metadata.mode();
            if !metadata.is_dir()
                || (metadata.uid() != uid && metadata.uid() != 0)
                || (mode & 0o022 != 0 && mode & 0o1000 == 0)
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "runtime socket parent directory is not protected against replacement",
                ));
            }
        }
        Ok(resolved)
    }

    fn ensure_private_directory(directory: &Path, uid: u32) -> io::Result<()> {
        match DirBuilder::new().mode(0o700).create(directory) {
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
            _ => {}
        }
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "runtime socket directory must be a real, owner-only directory owned by this user",
            ));
        }
        Ok(())
    }

    pub(super) fn listen(endpoint: &str) -> io::Result<Listener> {
        let path = socket_path(endpoint)?;
        let options = ListenerOptions::new().name(path.clone().to_fs_name::<GenericFilePath>()?);
        // Linux supports setting the socket mode before binding. On other Unix targets the
        // private directory already prevents access during the interval before chmod.
        #[cfg(target_os = "linux")]
        let options = options.mode(0o600);
        let listener = options.create_tokio()?;
        fs::set_permissions(&path, Permissions::from_mode(0o600))?;
        Ok(listener)
    }

    pub(super) fn reclaim(endpoint: &str) -> io::Result<()> {
        let path = socket_path(endpoint)?;
        match fs::symlink_metadata(&path) {
            Ok(metadata)
                if metadata.file_type().is_socket()
                    && metadata.uid() == rustix::process::geteuid().as_raw() =>
            {
                match fs::remove_file(path) {
                    Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
                    _ => Ok(()),
                }
            }
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "runtime endpoint is not a socket owned by this user",
            )),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{ensure_private_directory, trusted_temp_directory};
        use std::fs::{self, DirBuilder, Permissions};
        use std::io;
        use std::os::unix::fs::{symlink, DirBuilderExt, PermissionsExt};
        use std::path::PathBuf;

        struct Fixture(PathBuf);

        impl Fixture {
            fn new() -> Self {
                let path =
                    std::env::temp_dir().join(format!("brachio-dir-{}", uuid::Uuid::new_v4()));
                DirBuilder::new()
                    .mode(0o700)
                    .create(&path)
                    .expect("create fixture");
                Self(path)
            }
        }

        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        #[test]
        fn rejects_preexisting_symlink_and_shared_directory_without_changing_their_modes() {
            let fixture = Fixture::new();
            let uid = rustix::process::geteuid().as_raw();
            let directory = fixture.0.join("shared");
            DirBuilder::new()
                .mode(0o700)
                .create(&directory)
                .expect("create directory");
            fs::set_permissions(&directory, Permissions::from_mode(0o755))
                .expect("set shared mode");
            let private = fixture.0.join("private");
            DirBuilder::new()
                .mode(0o700)
                .create(&private)
                .expect("create private directory");
            let link = fixture.0.join("link");
            symlink(&private, &link).expect("create symbolic link");

            for path in [&link, &directory] {
                let error =
                    ensure_private_directory(path, uid).expect_err("reject insecure directory");
                assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            }
            assert_eq!(
                fs::metadata(&directory)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
            assert!(fs::symlink_metadata(&link)
                .expect("link metadata")
                .is_symlink());
        }

        #[test]
        fn rejects_a_directory_owned_by_a_different_uid() {
            let fixture = Fixture::new();
            // Compare against a different UID without needing chown or elevated permissions.
            let other_uid = rustix::process::geteuid().as_raw() ^ 1;
            let error =
                ensure_private_directory(&fixture.0, other_uid).expect_err("reject foreign owner");
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        }

        #[test]
        fn rejects_a_nonsticky_writable_parent() {
            let fixture = Fixture::new();
            fs::set_permissions(&fixture.0, Permissions::from_mode(0o777))
                .expect("set writable mode");
            let uid = rustix::process::geteuid().as_raw();
            let error =
                trusted_temp_directory(&fixture.0, uid).expect_err("reject replaceable parent");
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};

    use interprocess::local_socket::traits::tokio::Listener as _;

    use super::{connect, listen, platform, reclaim};
    use std::io;

    #[test]
    fn rejects_unsafe_unix_endpoint_names() {
        for endpoint in ["", "../escape", "nested/path", r"nested\\path"] {
            assert_eq!(
                listen(endpoint).err().map(|error| error.kind()),
                Some(io::ErrorKind::InvalidInput),
                "endpoint: {endpoint}"
            );
        }
    }

    #[tokio::test]
    async fn unix_socket_smoke_connects_and_reclaims() {
        let endpoint = format!("bs-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let listener = listen(&endpoint).expect("listen on owner-only socket");
        let path = platform::socket_path(&endpoint).expect("resolve socket path");
        let metadata = std::fs::symlink_metadata(path).expect("socket metadata");
        assert!(metadata.file_type().is_socket());
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        let client = connect(&endpoint).await.expect("connect to runtime");
        let server = listener.accept().await.expect("accept connection");
        drop(client);
        drop(server);
        drop(listener);
        reclaim(&endpoint).expect("reclaim socket");
    }
}
