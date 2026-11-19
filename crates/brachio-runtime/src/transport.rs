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
    platform::listen(endpoint, name(endpoint)?)
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
    use interprocess::local_socket::{ListenerOptions, Name};
    use interprocess::os::windows::local_socket::ListenerOptionsExt;
    use interprocess::os::windows::security_descriptor::SecurityDescriptor;
    use widestring::U16CString;

    pub(super) fn listen(_endpoint: &str, name: Name<'static>) -> io::Result<Listener> {
        let descriptor = U16CString::from_str("D:P(A;;GA;;;OW)").map_err(io::Error::other)?;
        let descriptor = SecurityDescriptor::deserialize(&descriptor)?;
        ListenerOptions::new()
            .name(name)
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
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::path::PathBuf;

    use interprocess::local_socket::tokio::Listener;
    use interprocess::local_socket::{ListenerOptions, Name};

    /// Unix socket paths are short (104 bytes on macOS), so the directory stays flat.
    const LONGEST_PATH: usize = 100;

    pub(super) fn socket_path(endpoint: &str) -> io::Result<PathBuf> {
        if endpoint.is_empty() || endpoint.contains(['/', '\\']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "endpoint must be a plain name",
            ));
        }
        let directory = std::env::temp_dir().join("brachio-runtime");
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&directory)?;
        // Changing the mode fails unless this user owns the directory.
        fs::set_permissions(&directory, Permissions::from_mode(0o700))?;
        let path = directory.join(format!("{endpoint}.sock"));
        if path.as_os_str().len() > LONGEST_PATH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "endpoint path is too long for a Unix socket",
            ));
        }
        Ok(path)
    }

    pub(super) fn listen(endpoint: &str, name: Name<'static>) -> io::Result<Listener> {
        let listener = ListenerOptions::new().name(name).create_tokio()?;
        fs::set_permissions(socket_path(endpoint)?, Permissions::from_mode(0o600))?;
        Ok(listener)
    }

    pub(super) fn reclaim(endpoint: &str) -> io::Result<()> {
        match fs::remove_file(socket_path(endpoint)?) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::{connect, listen, reclaim};
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
        let endpoint = format!("brachio-smoke-{}", uuid::Uuid::new_v4());
        let listener = listen(&endpoint).expect("listen on owner-only socket");
        let client = connect(&endpoint).await.expect("connect to runtime");
        let server = listener.accept().await.expect("accept connection");
        drop(client);
        drop(server);
        drop(listener);
        reclaim(&endpoint).expect("reclaim socket");
    }
}
