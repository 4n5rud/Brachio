//! Reconstructs the visible terminal frame independently of retained raw bytes.

use std::num::NonZeroU16;

pub(crate) const ROWS: NonZeroU16 = vt100::Parser::DEFAULT_ROWS;
pub(crate) const COLS: NonZeroU16 = vt100::Parser::DEFAULT_COLS;

pub(crate) struct TerminalScreen {
    parser: vt100::Parser,
    boundary: OutputBoundary,
}

pub(crate) struct Frame {
    pub(crate) ansi: String,
    pub(crate) cursor: (u16, u16),
    pub(crate) alternate_screen: bool,
    pub(crate) at_boundary: bool,
}

impl Default for TerminalScreen {
    fn default() -> Self {
        // Raw history has its own retention limit. The parser only needs its
        // active and inactive screens to produce a visible frame.
        Self {
            parser: vt100::Parser::new(ROWS, COLS, 0),
            boundary: OutputBoundary::default(),
        }
    }
}

impl TerminalScreen {
    pub(crate) fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
        self.boundary.process(bytes);
    }

    pub(crate) fn frame(&self) -> Frame {
        let screen = self.parser.screen();
        Frame {
            ansi: screen.state_formatted(),
            cursor: screen.cursor_position(),
            alternate_screen: screen.alternate_screen(),
            at_boundary: self.boundary.settled,
        }
    }
}

struct OutputBoundary {
    parser: vte::Parser,
    settled: bool,
}

impl Default for OutputBoundary {
    fn default() -> Self {
        Self {
            parser: vte::Parser::new(),
            settled: true,
        }
    }
}

impl OutputBoundary {
    fn process(&mut self, bytes: &[u8]) {
        let mut witness = BoundaryWitness {
            settled: false,
            allow_print: false,
        };
        let end = bytes
            .iter()
            .rposition(|byte| !matches!(byte, b'\r' | b'\n' | b'\t' | 8));
        if let Some(end) = end {
            self.parser.advance(&mut witness, &bytes[..end]);
            let final_byte = bytes[end];
            witness.settled = false;
            witness.allow_print = matches!(final_byte, 0x20..=0x7e | 0x80..=0xbf);
            self.parser.advance(&mut witness, &bytes[end..=end]);
            self.settled = witness.settled;
            self.parser.advance(&mut witness, &bytes[end + 1..]);
        } else {
            self.parser.advance(&mut witness, bytes);
        }
    }
}

struct BoundaryWitness {
    settled: bool,
    allow_print: bool,
}

impl vte::Perform for BoundaryWitness {
    fn print(&mut self, _: char) {
        self.settled = self.allow_print;
    }

    fn esc_dispatch(&mut self, _: &[u8], _: bool, _: u8) {
        self.settled = true;
    }

    fn csi_dispatch(&mut self, _: &vte::Params, _: &[u8], _: bool, _: char) {
        self.settled = true;
    }

    fn osc_dispatch(&mut self, _: &[&[u8]], bell_terminated: bool) {
        self.settled = bell_terminated;
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

    #[test]
    fn snapshots_only_mark_complete_utf8_and_escape_boundaries() {
        let mut screen = TerminalScreen::default();
        screen.process(b"ready\r\n");
        assert!(screen.frame().at_boundary);
        screen.process(&[0xed, 0x95]);
        assert!(!screen.frame().at_boundary);
        screen.process(&[0x9c]);
        assert!(screen.frame().at_boundary);
        screen.process(b"\x1b[31");
        assert!(!screen.frame().at_boundary);
        screen.process(b"m");
        assert!(screen.frame().at_boundary);
        screen.process(b"\x1b]0;title\x1b");
        assert!(!screen.frame().at_boundary);
        screen.process(b"\\");
        assert!(screen.frame().at_boundary);
        screen.process(&[0xc2, 0xe2]);
        assert!(!screen.frame().at_boundary);
        screen.process(b"\x1b");
        assert!(!screen.frame().at_boundary);
    }
}
