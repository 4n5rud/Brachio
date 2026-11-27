//! Protocol versions the runtime answers, so a runtime can outlive the app that started it.

/// The newest version this build speaks.
pub const CURRENT: u8 = 8;
/// The first version that offers a frame to follow from while a program is on the alternate
/// screen. An older runtime refuses `follow-frame` for as long as a full-screen program runs,
/// while a newer one refuses it only briefly, in the middle of an escape sequence.
pub const FULL_SCREEN_FRAMES: u8 = 8;
/// The oldest version this build still answers, so an older client keeps working.
pub const OLDEST: u8 = 1;

/// Whether this build answers requests written in `version`.
#[must_use]
pub fn accepts(version: u8) -> bool {
    (OLDEST..=CURRENT).contains(&version)
}

/// Tell the client which versions to retry with.
#[must_use]
pub fn rejection(version: u8) -> String {
    format!(
        "unsupported protocol version {version}; this runtime speaks {OLDEST} through {CURRENT}"
    )
}

/// Refuse an action that the client's chosen version did not define yet.
///
/// # Errors
/// Returns the message to send when `version` is older than `since`.
pub fn requires(version: u8, since: u8, name: &str) -> Result<(), String> {
    if version < since {
        return Err(format!(
            "{name} needs protocol version {since}, got {version}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{accepts, rejection, requires, CURRENT, OLDEST};

    #[test]
    fn accepts_only_the_supported_range() {
        assert!(accepts(OLDEST) && accepts(CURRENT));
        assert!(!accepts(0));
        assert!(!accepts(CURRENT + 1));
    }

    #[test]
    fn rejection_names_the_supported_range() {
        assert_eq!(
            rejection(9),
            "unsupported protocol version 9; this runtime speaks 1 through 8"
        );
    }

    #[test]
    fn newer_actions_are_refused_on_older_versions() {
        assert!(requires(1, 2, "drain").is_err());
        assert!(requires(2, 2, "drain").is_ok());
    }
}
