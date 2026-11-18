//! Tracks complete ANSI boundaries and detects mode 1047 switches.

pub(super) struct OutputBoundary {
    parser: vte::Parser,
    pub(super) settled: bool,
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
    pub(super) fn process(&mut self, bytes: &[u8]) -> Vec<(usize, bool)> {
        let mut changes = Vec::new();
        let mut witness = BoundaryWitness {
            settled: false,
            allow_print: false,
            alternate_change: None,
        };
        let end = bytes
            .iter()
            .rposition(|byte| !matches!(byte, b'\r' | b'\n' | b'\t' | 8));
        if let Some(end) = end {
            let mut consumed = 0;
            while consumed < end {
                consumed += self
                    .parser
                    .advance_until_terminated(&mut witness, &bytes[consumed..end]);
                if let Some(enter) = witness.alternate_change.take() {
                    changes.push((consumed, enter));
                }
            }
            let final_byte = bytes[end];
            witness.settled = false;
            witness.allow_print = matches!(final_byte, 0x20..=0x7e | 0x80..=0xbf);
            self.parser.advance(&mut witness, &bytes[end..=end]);
            if let Some(enter) = witness.alternate_change.take() {
                changes.push((end + 1, enter));
            }
            self.settled = witness.settled;
            self.parser.advance(&mut witness, &bytes[end + 1..]);
        } else {
            self.parser.advance(&mut witness, bytes);
        }
        changes
    }
}

struct BoundaryWitness {
    settled: bool,
    allow_print: bool,
    alternate_change: Option<bool>,
}

impl vte::Perform for BoundaryWitness {
    fn print(&mut self, _: char) {
        self.settled = self.allow_print;
    }

    fn esc_dispatch(&mut self, _: &[u8], _: bool, _: u8) {
        self.settled = true;
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignore: bool,
        action: char,
    ) {
        self.settled = true;
        if !ignore
            && intermediates == b"?"
            && matches!(action, 'h' | 'l')
            && params.iter().any(|param| param == [1047])
        {
            self.alternate_change = Some(action == 'h');
        }
    }

    fn terminated(&self) -> bool {
        self.alternate_change.is_some()
    }

    fn osc_dispatch(&mut self, _: &[&[u8]], bell_terminated: bool) {
        self.settled = bell_terminated;
    }
}
