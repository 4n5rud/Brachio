//! Reconstructs the visible terminal frame independently of retained raw bytes.

use std::num::NonZeroU16;

pub(crate) const ROWS: NonZeroU16 = vt100::Parser::DEFAULT_ROWS;
pub(crate) const COLS: NonZeroU16 = vt100::Parser::DEFAULT_COLS;

pub(crate) struct TerminalScreen(vt100::Parser);

pub(crate) struct Frame {
    pub(crate) ansi: String,
    pub(crate) cursor: (u16, u16),
    pub(crate) alternate_screen: bool,
}

impl Default for TerminalScreen {
    fn default() -> Self {
        // Raw history has its own retention limit. The parser only needs its
        // active and inactive screens to produce a visible frame.
        Self(vt100::Parser::new(ROWS, COLS, 0))
    }
}

impl TerminalScreen {
    pub(crate) fn process(&mut self, bytes: &[u8]) {
        self.0.process(bytes);
    }

    pub(crate) fn frame(&self) -> Frame {
        let screen = self.0.screen();
        Frame {
            ansi: screen.state_formatted(),
            cursor: screen.cursor_position(),
            alternate_screen: screen.alternate_screen(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TerminalScreen, COLS, ROWS};

    #[test]
    fn frame_renders_wide_text_and_cursor_without_replaying_queries() {
        let mut screen = TerminalScreen::default();
        let bytes = "한글".as_bytes();
        screen.process(&bytes[..2]);
        screen.process(&bytes[2..]);
        screen.process(b"\x1b[");
        screen.process(b"3;6H\x1b[6n\x1b[?2004h");

        let frame = screen.frame();
        assert_eq!(frame.cursor, (2, 5));
        assert!(!frame.alternate_screen);
        assert!(!frame.ansi.contains("\x1b[6n"));
        let mut restored = vt100::Parser::new(ROWS, COLS, 0);
        restored.process(frame.ansi.as_bytes());
        assert!(restored.screen().contents().contains("한글"));
        assert_eq!(restored.screen().cursor_position(), frame.cursor);
        assert!(restored.screen().bracketed_paste());
    }

    #[test]
    fn identifies_which_buffer_the_visible_frame_represents() {
        let mut screen = TerminalScreen::default();
        screen.process(b"normal\x1b[?1049hfull screen");
        let full = screen.frame();
        assert!(full.alternate_screen);
        assert!(full.ansi.contains("full screen"));

        screen.process(b"\x1b[?1049l");
        let normal = screen.frame();
        assert!(!normal.alternate_screen);
        assert!(normal.ansi.contains("normal"));
    }
}
