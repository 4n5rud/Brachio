//! Decides from a connection attempt whether a runtime endpoint is in use.

use std::io;

/// Exit status when another runtime is already serving the endpoint.
pub const EXIT_OCCUPIED: i32 = 75;
/// Exit status when the endpoint could not be shown to be free or in use.
pub const EXIT_UNCERTAIN: i32 = 76;

/// What a connection attempt shows about the owner of an endpoint.
#[derive(Debug, PartialEq, Eq)]
pub enum Liveness {
    /// A runtime accepted the connection.
    Serving,
    /// Nothing serves the endpoint.
    Dead,
    /// The attempt proves nothing either way.
    Unknown,
}

/// Only a refused or missing endpoint proves nothing serves it. A timeout, a busy
/// pipe or a permission error proves nothing, and must never read as dead.
pub fn classify<T>(attempt: &io::Result<T>) -> Liveness {
    match attempt {
        Ok(_) => Liveness::Serving,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            Liveness::Dead
        }
        Err(_) => Liveness::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::{classify, Liveness};
    use std::io::{Error, ErrorKind};

    #[test]
    fn only_a_connection_proves_use_and_only_refusal_proves_death() {
        assert_eq!(classify(&Ok(())), Liveness::Serving);
        for kind in [ErrorKind::NotFound, ErrorKind::ConnectionRefused] {
            assert_eq!(classify::<()>(&Err(Error::from(kind))), Liveness::Dead);
        }
        for kind in [
            ErrorKind::TimedOut,
            ErrorKind::PermissionDenied,
            ErrorKind::WouldBlock,
            ErrorKind::Other,
        ] {
            assert_eq!(classify::<()>(&Err(Error::from(kind))), Liveness::Unknown);
        }
    }
}
