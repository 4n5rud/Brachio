//! What the runtime keeps of programs that are gone: their last screen, in memory and on disk.

use std::collections::HashMap;

use anyhow::{Context, Result};
use brachio_runtime::wire::{LostInfo, ResponseResult, ScreenSnapshot};

use crate::journal::{Journal, SessionRecord};
use crate::runtime::Runtime;
use crate::session::{lock, lost_info, Session};

impl Journal {
    /// Records left by runtimes that are gone. Unreadable files are reported and set aside.
    pub(crate) fn read_lost(&self) -> HashMap<String, SessionRecord> {
        match self.load() {
            Ok(loaded) => {
                for path in &loaded.unreadable {
                    eprintln!("set aside unreadable journal file {}", path.display());
                }
                loaded
                    .records
                    .into_iter()
                    .map(|record| (record.key.clone(), record))
                    .collect()
            }
            Err(error) => {
                eprintln!("cannot read the journal: {error}");
                HashMap::new()
            }
        }
    }
}

impl Runtime {
    /// Write the screen of every shell that changed since the last write.
    pub(crate) fn persist(&self) {
        let Some(journal) = &self.journal else { return };
        let (candidates, mut saved) = {
            let Ok(sessions) = lock(&self.sessions) else {
                return;
            };
            let Ok(saved) = lock(&self.saved) else {
                return;
            };
            let candidates: Vec<_> = sessions
                .iter()
                .map(|(key, session)| (key.clone(), session.clone()))
                .collect();
            // Keep journal ordering with stop and exit, but allow requests to the other
            // sessions while changed frames are formatted and written.
            (candidates, saved)
        };
        for (key, session) in candidates {
            let Ok(mark) = session.mark() else { continue };
            if saved.get(&key) == Some(&mark) {
                continue;
            }
            if let Ok((record, mark)) = session.record(&key, &self.instance) {
                if journal.save(&record).is_ok() {
                    saved.insert(key, mark);
                }
            }
        }
    }

    /// Move every program that ended by itself out of the live set, keeping its last screen.
    pub(crate) fn keep_ended_programs(
        &self,
        sessions: &mut HashMap<String, Session>,
    ) -> Result<()> {
        let active = self.manager.ids()?;
        let ended: Vec<String> = sessions
            .iter()
            .filter(|(_, session)| !active.contains(&session.id))
            .map(|(key, _)| key.clone())
            .collect();
        for key in ended {
            if let Some(session) = sessions.remove(&key) {
                if let Err(error) = self.keep_ended(&key, &session) {
                    eprintln!("cannot keep the last screen of {key}: {error:#}");
                }
            }
        }
        Ok(())
    }

    /// A shell that ended by itself keeps its last screen, unless the user stopped it.
    fn keep_ended(&self, key: &str, session: &Session) -> Result<()> {
        if lock(&self.stopped)?.remove(key) {
            return Ok(());
        }
        let (mut record, _) = session.record(key, &self.instance)?;
        record.exited = true;
        {
            let mut saved = lock(&self.saved)?;
            if let Some(journal) = &self.journal {
                journal.save(&record)?;
            }
            saved.remove(key);
        }
        lock(&self.lost)?.insert(key.to_owned(), record);
        Ok(())
    }

    pub(crate) fn forget(&self, key: &str) -> Result<()> {
        lock(&self.lost)?.remove(key);
        let mut saved = lock(&self.saved)?;
        if let Some(journal) = &self.journal {
            journal.remove(key)?;
        }
        saved.remove(key);
        Ok(())
    }

    pub(crate) fn list_lost(&self) -> Result<ResponseResult> {
        let lost = lock(&self.lost)?;
        let mut listed: Vec<LostInfo> = lost.values().map(lost_info).collect();
        listed.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(ResponseResult::Lost { lost: listed })
    }

    pub(crate) fn recover(&self, key: &str) -> Result<ResponseResult> {
        let lost = lock(&self.lost)?;
        let record = lost
            .get(key)
            .with_context(|| format!("no saved screen for {key}"))?;
        Ok(ResponseResult::Recovered {
            lost: lost_info(record),
            screen: ScreenSnapshot {
                ansi: record.ansi.clone(),
                cursor: record.cursor,
                rows: record.rows,
                cols: record.cols,
                alternate_screen: record.alternate_screen,
                resume_offset: None,
            },
        })
    }
}
