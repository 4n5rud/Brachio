//! Reconstructs the visible terminal frame independently of retained raw bytes.

use std::num::NonZeroU16;

pub(crate) const ROWS: NonZeroU16 = vt100::Parser::DEFAULT_ROWS;
pub(crate) const COLS: NonZeroU16 = vt100::Parser::DEFAULT_COLS;

/// Escape sequences that switch to the alternate screen.
const ALTERNATE_ENTRIES: [&[u8]; 3] = [b"[?1049h", b"[?1047h", b"[?47h"];
const LONGEST_ENTRY: usize = 8;

pub(crate) struct TerminalScreen {
    parser: vt100::Parser,
    boundary: OutputBoundary,
    size: (NonZeroU16, NonZeroU16),
    /// The normal screen as it was when the alternate screen was entered.
    normal_frame: Option<String>,
    /// Trailing bytes of the previous chunk, in case an entry sequence is split.
    tail: Vec<u8>,
}

pub(crate) struct Frame {
    pub(crate) ansi: String,
    pub(crate) cursor: (u16, u16),
    pub(crate) rows: u16,
    pub(crate) cols: u16,
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
            size: (ROWS, COLS),
            normal_frame: None,
            tail: Vec::new(),
        }
    }
}

impl TerminalScreen {
    pub(crate) fn process(&mut self, bytes: &[u8]) {
        self.boundary.process(bytes);
        let mut rest = bytes;
        let mut tail = self.tail.clone();
        while let Some(entry) = self.next_entry(&tail, rest) {
            tail.clear();
            self.parser.process(&rest[..entry.start]);
            self.normal_frame = Some(self.parser.screen().state_formatted());
            let end = entry.start + entry.length;
            self.parser.process(&rest[entry.start..end]);
            rest = &rest[end..];
        }
        self.parser.process(rest);
        if !self.parser.screen().alternate_screen() {
            self.normal_frame = None;
        }
        self.remember_tail(bytes);
    }

    /// Change the emulated size so later frames match the PTY.
    pub(crate) fn resize(&mut self, rows: NonZeroU16, cols: NonZeroU16) {
        self.parser.set_size(rows, cols);
        self.size = (rows, cols);
    }

    /// Find the next switch away from the normal screen, even when it straddles two chunks.
    fn next_entry(&self, tail: &[u8], bytes: &[u8]) -> Option<AlternateEntry> {
        if self.parser.screen().alternate_screen() {
            return None;
        }
        let joined: Vec<u8> = tail.iter().chain(bytes).copied().collect();
        ALTERNATE_ENTRIES
            .iter()
            .filter_map(|entry| {
                let at = joined
                    .windows(entry.len())
                    .position(|window| window == *entry)?;
                let end = (at + entry.len())
                    .checked_sub(tail.len())
                    .filter(|end| *end > 0)?;
                let start = at.saturating_sub(tail.len());
                Some(AlternateEntry {
                    start,
                    length: end - start,
                })
            })
            .min_by_key(|entry| entry.start)
    }

    fn remember_tail(&mut self, bytes: &[u8]) {
        self.tail.extend_from_slice(bytes);
        let keep = self.tail.len().saturating_sub(LONGEST_ENTRY - 1);
        self.tail.drain(..keep);
    }

    pub(crate) fn frame(&self) -> Frame {
        let screen = self.parser.screen();
        let mut ansi = String::new();
        if let Some(normal) = &self.normal_frame {
            ansi.push_str(normal);
            ansi.push_str("[?1049h");
        }
        ansi.push_str(&screen.state_formatted());
        Frame {
            ansi,
            cursor: screen.cursor_position(),
            rows: self.size.0.get(),
            cols: self.size.1.get(),
            alternate_screen: screen.alternate_screen(),
            at_boundary: self.boundary.settled,
        }
    }
}

struct AlternateEntry {
    start: usize,
    length: usize,
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

    #[test]
    fn frame_restores_the_normal_screen_behind_the_alternate_one() {
        let mut screen = TerminalScreen::default();
        screen.process(
            b"prompt$ ls
file.txt
[?10",
        );
        screen.process(b"49hfull screen");
        let frame = screen.frame();
        assert!(frame.alternate_screen);

        let mut restored = vt100::Parser::new(ROWS, COLS, 0);
        restored.process(frame.ansi.as_bytes());
        assert!(restored.screen().alternate_screen());
        assert!(restored.screen().contents().contains("full screen"));
        restored.process(b"[?1049l");
        assert!(restored.screen().contents().contains("file.txt"));

        screen.process(b"[?1049lback");
        assert!(!screen.frame().ansi.contains("[?1049h"));
    }

    #[test]
    fn resize_changes_the_reported_frame_size() {
        let mut screen = TerminalScreen::default();
        let rows = std::num::NonZeroU16::new(40).expect("nonzero");
        let cols = std::num::NonZeroU16::new(120).expect("nonzero");
        screen.resize(rows, cols);
        screen.process("a".repeat(100).as_bytes());
        let frame = screen.frame();
        assert_eq!((frame.rows, frame.cols), (40, 120));
        assert_eq!(frame.cursor, (0, 100));
    }
}
