//! Limits on messages between agent sessions, shared by the app that stores them and the
//! commands that send them.

/// Longest message body, in bytes. Anything bigger belongs in a file the message points to.
pub const MAX_BODY_BYTES: usize = 8 * 1024;
