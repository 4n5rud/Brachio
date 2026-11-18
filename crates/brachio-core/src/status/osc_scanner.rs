//! Finds OSC 7501 sequences in a terminal output stream.

/// Bytes after ESC that open an OSC 7501 sequence.
const PREFIX: &[u8] = b"]7501;";

/// Longest body the protocol allows: 4096 bytes minus the framing.
const MAX_BODY_BYTES: usize = 4087;

const ESCAPE: u8 = 0x1b;
const BELL: u8 = 0x07;

/// Pulls OSC 7501 bodies out of PTY output.
///
/// Output arrives in arbitrary chunks, so a sequence can start in one chunk
/// and end in the next. The scanner keeps its place between calls. It only
/// reads the stream; the bytes still go to the terminal unchanged.
#[derive(Debug, Default)]
pub struct OscScanner {
    state: ScanState,
    body: Vec<u8>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum ScanState {
    /// Outside any sequence we care about.
    #[default]
    Ground,
    /// Matched ESC and this many bytes of `PREFIX`.
    Prefix(usize),
    /// Inside the body.
    Body,
    /// Inside the body, right after an ESC that may start the terminator.
    BodyEscape,
    /// Inside a body that went over the size limit, waiting for its end.
    Overlong,
}

impl OscScanner {
    /// Creates a scanner outside any sequence.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Scans one chunk and calls `on_body` for each complete body.
    ///
    /// A body that is not valid UTF-8 or is too long is dropped.
    pub fn feed(&mut self, bytes: &[u8], mut on_body: impl FnMut(&str)) {
        for &byte in bytes {
            self.step(byte, &mut on_body);
        }
    }

    fn step(&mut self, byte: u8, on_body: &mut impl FnMut(&str)) {
        self.state = match self.state {
            ScanState::Ground => Self::after_ground(byte),
            ScanState::Prefix(_) => {
                let next = self.state.advance(byte);
                if next == ScanState::Body {
                    self.body.clear();
                }
                next
            }
            ScanState::Body => match byte {
                BELL => self.finish(on_body),
                ESCAPE => ScanState::BodyEscape,
                _ if self.body.len() >= MAX_BODY_BYTES => ScanState::Overlong,
                _ => {
                    self.body.push(byte);
                    ScanState::Body
                }
            },
            ScanState::BodyEscape if byte == b'\\' => self.finish(on_body),
            // Any other byte after ESC aborts the body. That ESC may open a new sequence.
            ScanState::BodyEscape => ScanState::Prefix(0).advance(byte),
            ScanState::Overlong => match byte {
                BELL => ScanState::Ground,
                ESCAPE => ScanState::Prefix(0),
                _ => ScanState::Overlong,
            },
        };
    }

    fn after_ground(byte: u8) -> ScanState {
        ScanState::Ground.advance(byte)
    }

    fn finish(&mut self, on_body: &mut impl FnMut(&str)) -> ScanState {
        if let Ok(body) = std::str::from_utf8(&self.body) {
            on_body(body);
        }
        self.body.clear();
        ScanState::Ground
    }
}

impl ScanState {
    /// Next state while matching ESC and `PREFIX`.
    fn advance(self, byte: u8) -> Self {
        match self {
            Self::Prefix(matched) if PREFIX.get(matched) == Some(&byte) => {
                if matched + 1 == PREFIX.len() {
                    Self::Body
                } else {
                    Self::Prefix(matched + 1)
                }
            }
            _ if byte == ESCAPE => Self::Prefix(0),
            _ => Self::Ground,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bodies(chunks: &[&[u8]]) -> Vec<String> {
        let mut scanner = OscScanner::new();
        let mut found = Vec::new();
        for chunk in chunks {
            scanner.feed(chunk, |body| found.push(body.to_owned()));
        }
        found
    }

    #[test]
    fn finds_bodies_ended_by_st_or_bell() {
        let found = bodies(&[b"hi\x1b]7501;state=working\x1b\\ text \x1b]7501;?\x07"]);
        assert_eq!(found, ["state=working", "?"]);
    }

    #[test]
    fn joins_a_sequence_split_across_chunks() {
        let found = bodies(&[b"\x1b]75", b"01;state=", b"idle\x1b", b"\\"]);
        assert_eq!(found, ["state=idle"]);
    }

    #[test]
    fn ignores_other_sequences() {
        let found = bodies(&[b"\x1b]0;title\x07\x1b[31mred\x1b]7502;x\x07"]);
        assert!(found.is_empty());
    }

    #[test]
    fn restarts_on_escape_inside_a_body() {
        let found = bodies(&[b"\x1b]7501;broken\x1b]7501;state=done\x1b\\"]);
        assert_eq!(found, ["state=done"]);
    }

    #[test]
    fn drops_overlong_bodies() {
        let mut stream = b"\x1b]7501;".to_vec();
        stream.extend(std::iter::repeat_n(b'a', MAX_BODY_BYTES + 10));
        stream.extend(b"\x07\x1b]7501;state=idle\x07");
        let found = bodies(&[&stream]);
        assert_eq!(found, ["state=idle"]);
    }
}
