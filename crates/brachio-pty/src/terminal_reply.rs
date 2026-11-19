//! Getting a terminal's answer to a program's query into the program's input.
//!
//! On Unix an answer is simply written to the PTY. A Windows pseudo console does not pass every
//! escape sequence it is given on: it hands text and ordinary key sequences (such as the cursor
//! position report) to the program but drops operating system command strings, so the answer to
//! the `OSC 7501 ; ?` probe never arrived and agents never started reporting their status. The
//! same text written as Windows key events, one event per character, reaches the program intact.

/// The bytes to write to a PTY so that the program reads `reply` from its input.
#[must_use]
pub(crate) fn input_for_reply(reply: &[u8]) -> Vec<u8> {
    if cfg!(windows) {
        as_windows_key_events(reply)
    } else {
        reply.to_vec()
    }
}

/// Each character as a key press and release in win32-input-mode:
/// `ESC [ Vk ; Sc ; Uc ; Kd ; Cs ; Rc _`, with only the character code `Uc` set.
fn as_windows_key_events(reply: &[u8]) -> Vec<u8> {
    let mut events = Vec::with_capacity(reply.len() * 28);
    for &byte in reply {
        for pressed in [1, 0] {
            events.extend_from_slice(format!("\x1b[0;0;{byte};{pressed};0;1_").as_bytes());
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::{as_windows_key_events, input_for_reply};

    #[test]
    fn each_character_becomes_a_key_press_and_a_release() {
        assert_eq!(
            as_windows_key_events(b"a]"),
            b"\x1b[0;0;97;1;0;1_\x1b[0;0;97;0;0;1_\x1b[0;0;93;1;0;1_\x1b[0;0;93;0;0;1_"
        );
        let escape = as_windows_key_events(b"\x1b");
        assert!(escape.starts_with(b"\x1b[0;0;27;1;0;1_"));
    }

    #[test]
    fn only_windows_changes_how_the_reply_is_written() {
        let reply = b"\x1b]7501;?\x1b\\";
        if cfg!(windows) {
            assert_eq!(input_for_reply(reply), as_windows_key_events(reply));
        } else {
            assert_eq!(input_for_reply(reply), reply.to_vec());
        }
    }
}
