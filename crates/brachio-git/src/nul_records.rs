//! Splits a stream of NUL-terminated records that arrives in arbitrary pieces.

/// A single path or status line longer than this means the output is not what was expected.
const LONGEST_RECORD: usize = 1024 * 1024;

#[derive(Default)]
pub(crate) struct NulRecords {
    partial: Vec<u8>,
    too_long: bool,
}

impl NulRecords {
    pub(crate) fn push(&mut self, chunk: &[u8], mut on_record: impl FnMut(&str)) {
        for piece in chunk.split_inclusive(|byte| *byte == 0) {
            let Some((&0, body)) = piece.split_last() else {
                self.append(piece);
                continue;
            };
            if self.partial.is_empty() {
                on_record(&String::from_utf8_lossy(body));
            } else {
                self.append(body);
                on_record(&String::from_utf8_lossy(&self.partial));
                self.partial.clear();
            }
        }
    }

    fn append(&mut self, bytes: &[u8]) {
        if self.partial.len() + bytes.len() > LONGEST_RECORD {
            self.too_long = true;
            self.partial.clear();
        } else if !self.too_long {
            self.partial.extend_from_slice(bytes);
        }
    }

    /// Delivers a last record that had no terminator.
    ///
    /// # Errors
    /// Fails when a record was longer than any real path.
    pub(crate) fn finish(self, mut on_record: impl FnMut(&str)) -> Result<(), String> {
        if self.too_long {
            return Err("git printed a record longer than 1 MiB".to_owned());
        }
        if !self.partial.is_empty() {
            on_record(&String::from_utf8_lossy(&self.partial));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(chunks: &[&[u8]]) -> Vec<String> {
        let mut records = Vec::new();
        let mut splitter = NulRecords::default();
        for chunk in chunks {
            splitter.push(chunk, |record| records.push(record.to_owned()));
        }
        splitter
            .finish(|record| records.push(record.to_owned()))
            .expect("short records");
        records
    }

    #[test]
    fn records_split_across_chunks_stay_whole() {
        let records = split(&[b"a.txt\0\xed\x95", b"\x9c.txt\0", b"b"]);
        assert_eq!(records, ["a.txt", "한.txt", "b"]);
    }

    #[test]
    fn a_record_longer_than_any_path_is_an_error() {
        let mut splitter = NulRecords::default();
        let chunk = vec![b'x'; 512 * 1024];
        for _ in 0..3 {
            splitter.push(&chunk, |_| {});
        }
        assert!(splitter.finish(|_| {}).is_err());
    }
}
