//! The session's event log (design §10): `events.jsonl`, one line per
//! request, rotated to `events.1.jsonl` past a size limit. Reads go from the
//! newest line backwards, so asking for the last few never reads the whole
//! log.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use gbcli_protocol::{EventLogLine, Rev};

use crate::error::StoreError;
use crate::io::{append, io_at};
use crate::store::SessionStore;

/// Where the log rotates (design §10).
pub const EVENTS_ROTATE_AT: u64 = 10 * 1024 * 1024;

/// Bytes read per step when reading backwards.
const CHUNK: u64 = 64 * 1024;

impl SessionStore {
    fn events_path(&self) -> PathBuf {
        self.dir().join("events.jsonl")
    }

    fn rotated_events_path(&self) -> PathBuf {
        self.dir().join("events.1.jsonl")
    }

    /// Appends lines, first rotating the log if they would take it past the
    /// limit. The previous rotation is replaced: one old file is kept.
    pub fn append_events(&self, lines: &[EventLogLine]) -> Result<(), StoreError> {
        let path = self.events_path();
        let mut text = String::new();
        for line in lines {
            text.push_str(&serde_json::to_string(line).expect("an event line serializes"));
            text.push('\n');
        }
        let size = fs::metadata(&path).map_or(0, |m| m.len());
        if size > 0 && size + text.len() as u64 > self.events_rotate_at() {
            let rotated = self.rotated_events_path();
            fs::rename(&path, &rotated).map_err(io_at(&rotated))?;
        }
        append(&path, &text)
    }

    /// The newest `limit` lines, oldest first; every kept line without one.
    pub fn events(&self, limit: Option<usize>) -> Result<Vec<EventLogLine>, StoreError> {
        let mut rows = Vec::new();
        for row in self.events_newest_first() {
            if limit.is_some_and(|limit| rows.len() >= limit) {
                break;
            }
            rows.push(row?);
        }
        rows.reverse();
        Ok(rows)
    }

    /// Who moved the session to `rev`: the newest line that changed the
    /// revision to it. Reads and refusals leave the revision alone, so they
    /// never hide the writer, however many there are since. `None` when no
    /// kept line did, or the writer named no actor.
    pub fn last_writer(&self, rev: Rev) -> Result<Option<String>, StoreError> {
        for row in self.events_newest_first() {
            let row = row?;
            if row.rev_after == rev && row.rev_before != row.rev_after {
                return Ok(row.actor);
            }
        }
        Ok(None)
    }

    /// Every kept line, newest first, across the current and rotated files.
    /// A line that doesn't parse (another version wrote it) is skipped.
    fn events_newest_first(&self) -> impl Iterator<Item = Result<EventLogLine, StoreError>> {
        type Lines = Box<dyn Iterator<Item = Result<Vec<u8>, StoreError>>>;
        // Lazy: the rotated file is opened only if the current one runs out.
        [self.events_path(), self.rotated_events_path()]
            .into_iter()
            .flat_map(|path| -> Lines {
                match Backward::open(&path) {
                    Ok(lines) => Box::new(lines),
                    Err(e) => Box::new(std::iter::once(Err(e))),
                }
            })
            .filter_map(|line| match line {
                Ok(bytes) => serde_json::from_slice(&bytes).ok().map(Ok),
                Err(e) => Some(Err(e)),
            })
    }
}

/// A file's lines from the last to the first, read in chunks from the end.
struct Backward {
    path: PathBuf,
    file: Option<File>,
    /// Bytes before this offset are unread.
    pos: u64,
    /// The start of a line whose beginning is in an unread chunk.
    partial: Vec<u8>,
    /// Whole lines from the last chunk, in file order.
    ready: Vec<Vec<u8>>,
}

impl Backward {
    /// A missing file has no lines.
    fn open(path: &Path) -> Result<Backward, StoreError> {
        let (file, pos) = match File::open(path) {
            Ok(file) => {
                let len = file.metadata().map_err(io_at(path))?.len();
                (Some(file), len)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (None, 0),
            Err(source) => {
                return Err(StoreError::Io {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        Ok(Backward {
            path: path.to_path_buf(),
            file,
            pos,
            partial: Vec::new(),
            ready: Vec::new(),
        })
    }

    fn read_chunk(&mut self) -> Result<(), StoreError> {
        let Some(file) = self.file.as_mut() else {
            return Ok(());
        };
        let start = self.pos.saturating_sub(CHUNK);
        let mut chunk = vec![0; (self.pos - start) as usize];
        file.seek(SeekFrom::Start(start))
            .and_then(|_| file.read_exact(&mut chunk))
            .map_err(io_at(&self.path))?;
        self.pos = start;
        chunk.append(&mut self.partial);
        let mut parts = chunk.split(|&b| b == b'\n');
        // Everything before the chunk's first newline may continue further back.
        self.partial = parts.next().unwrap_or_default().to_vec();
        self.ready = parts
            .filter(|p| !p.is_empty())
            .map(<[u8]>::to_vec)
            .collect();
        Ok(())
    }
}

impl Iterator for Backward {
    type Item = Result<Vec<u8>, StoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(line) = self.ready.pop() {
                return Some(Ok(line));
            }
            if self.pos == 0 {
                let line = std::mem::take(&mut self.partial);
                return (!line.is_empty()).then_some(Ok(line));
            }
            if let Err(e) = self.read_chunk() {
                self.pos = 0;
                return Some(Err(e));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use gbcli_protocol::{EventsArgs, Op};

    use super::*;
    use crate::test_fixture::Fixture;

    fn line(actor: &str, op: Op, before: u64, after: u64) -> EventLogLine {
        EventLogLine {
            at: 0,
            actor: Some(actor.into()),
            op,
            ok: true,
            code: None,
            rev_before: Rev(before),
            rev_after: Rev(after),
            gaps: BTreeMap::new(),
            stop: None,
        }
    }

    fn read(actor: &str, rev: u64) -> EventLogLine {
        line(actor, Op::Show, rev, rev)
    }

    #[test]
    fn the_log_rotates_past_its_limit_and_still_reads_the_newest() {
        let f = Fixture::new("events-rotate");
        let mut store = f.demo();
        store.set_events_rotate_at(512);
        for rev in 0..40 {
            store.append_events(&[read("bob", rev)]).unwrap();
        }
        assert!(store.rotated_events_path().is_file(), "never rotated");
        let size = fs::metadata(store.events_path()).unwrap().len();
        assert!(size <= 512, "{size} bytes");
        let newest = store.events(Some(3)).unwrap();
        let revs: Vec<u64> = newest.iter().map(|row| row.rev_after.0).collect();
        assert_eq!(revs, vec![37, 38, 39]);
    }

    #[test]
    fn reading_backwards_crosses_chunks_and_files() {
        let f = Fixture::new("events-chunks");
        let mut store = f.demo();
        store.set_events_rotate_at(CHUNK * 2);
        let rows: Vec<EventLogLine> = (0..6000).map(|rev| read("bob", rev)).collect();
        for chunk in rows.chunks(100) {
            store.append_events(chunk).unwrap();
        }
        let all = store.events(None).unwrap();
        let revs: Vec<u64> = all.iter().map(|row| row.rev_after.0).collect();
        let first = revs[0];
        assert_eq!(
            revs,
            (first..6000).collect::<Vec<_>>(),
            "a gap or a reorder"
        );
        assert!(first > 0, "rotation kept everything");
    }

    #[test]
    fn the_last_writer_is_found_behind_any_number_of_readers() {
        let f = Fixture::new("events-writer");
        let store = f.demo();
        store
            .append_events(&[line("alice", Op::Undo, 1, 2)])
            .unwrap();
        let readers: Vec<EventLogLine> = (0..500).map(|_| read("bob", 2)).collect();
        store.append_events(&readers).unwrap();
        assert_eq!(store.last_writer(Rev(2)).unwrap().as_deref(), Some("alice"));
        assert_eq!(store.last_writer(Rev(3)).unwrap(), None);
    }

    #[test]
    fn an_event_line_keeps_the_args_it_was_given() {
        let f = Fixture::new("events-args");
        let store = f.demo();
        let op = Op::Events(EventsArgs { limit: Some(4) });
        store
            .append_events(&[line("bob", op.clone(), 1, 1)])
            .unwrap();
        assert_eq!(store.events(None).unwrap()[0].op, op);
    }

    #[test]
    fn appending_to_a_removed_session_does_not_recreate_it() {
        let f = Fixture::new("events-removed");
        let store = f.demo();
        fs::remove_dir_all(store.dir()).unwrap();
        assert!(store.append_events(&[read("bob", 0)]).is_err());
        assert!(!store.dir().exists());
    }
}
