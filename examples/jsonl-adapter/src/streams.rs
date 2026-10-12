/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
// Third Party Imports
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
// Local Imports
use harmos::{Application, Batch, Cursor, Definition, Stream, StreamConsumer};

/* === Constants === */
const CHECKPOINT: &str = ".harmos-jsonl.cursor";

const CHECKPOINT_TEMP: &str = ".harmos-jsonl.cursor.tmp";

const GAP: &str = "gap_missed";

const POSITION: &str = "position";

const SUFFIX: &str = ".jsonl";

const WINDOW: usize = 64 * 1024;

/* === Macros === */

/* === Type Aliases === */
type Encode<A> = fn(&Batch<A>) -> io::Result<Vec<Map<String, Value>>>;

/* === Structs === */
pub struct JsonlStreams<A: Application> {
    directory: PathBuf,
    cursor: Cursor,
    selections: Vec<Selection<A>>,
    faulted: bool,
}

struct Selection<A: Application> {
    definition: Definition,
    encode: Encode<A>,
    writer: Option<File>,
    pending_gap: u64,
    application: PhantomData<fn() -> A>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<A: Application> JsonlStreams<A> {
    pub fn open(directory: impl AsRef<Path>) -> io::Result<Self> {
        let directory = directory.as_ref().to_owned();
        std::fs::create_dir_all(&directory)?;
        heal(&directory)?;

        Ok(Self {
            cursor: read_checkpoint(&directory)?,
            directory,
            selections: Vec::new(),
            faulted: false,
        })
    }

    pub fn select<S: Stream<A>>(mut self) -> Self {
        if self
            .selections
            .iter()
            .any(|selected| selected.definition.id() == S::ID)
        {
            return self;
        }
        self.selections.push(Selection::of::<S>());

        self
    }

    fn capture(&mut self, batch: &Batch<A>) -> io::Result<()> {
        let mut encoded = Vec::with_capacity(self.selections.len());
        let mut delivered = 0_u64;

        for selection in &self.selections {
            let rows = (selection.encode)(batch)?;
            delivered += rows.len() as u64;
            encoded.push(rows);
        }

        let crossed = batch.reaches().0.saturating_sub(self.cursor.0);
        let missed = crossed.saturating_sub(delivered);
        for (selection, mut rows) in self.selections.iter_mut().zip(encoded) {
            selection.pending_gap = selection.pending_gap.saturating_add(missed);
            let Some(first) = rows.first_mut() else {
                continue;
            };
            if selection.pending_gap > 0 {
                // Evidence on the first real row after loss; no row is invented.
                first.insert(GAP.to_owned(), Value::from(selection.pending_gap));
                selection.pending_gap = 0;
            }
            selection.write(&self.directory, &rows)?;
        }

        Ok(())
    }

    fn finalize(&mut self) -> io::Result<()> {
        for selection in &mut self.selections {
            selection.close()?;
        }

        Ok(())
    }
}

impl<A: Application> StreamConsumer<A> for JsonlStreams<A> {
    fn resume(&self) -> Cursor {
        self.cursor
    }

    fn selections(&self) -> Vec<Definition> {
        self.selections
            .iter()
            .map(|selected| selected.definition.clone())
            .collect()
    }

    async fn drain(&mut self, batch: Batch<A>) -> Cursor {
        if self.faulted {
            return self.cursor;
        }
        if self.capture(&batch).is_err()
            || persist_checkpoint(&self.directory, batch.reaches()).is_err()
        {
            self.faulted = true;
            return self.cursor;
        }

        self.cursor = batch.reaches();
        self.cursor
    }

    async fn flush(&mut self) {
        if !self.faulted && self.finalize().is_err() {
            self.faulted = true;
        }
    }
}

impl<A: Application> Selection<A> {
    fn of<S: Stream<A>>() -> Self {
        Self {
            definition: S::definition(),
            encode: encode::<A, S>,
            writer: None,
            pending_gap: 0,
            application: PhantomData,
        }
    }

    fn write(&mut self, directory: &Path, rows: &[Map<String, Value>]) -> io::Result<()> {
        if self.writer.is_none() {
            let path = capture_path(directory, self.definition.id());
            self.writer = Some(OpenOptions::new().create(true).append(true).open(path)?);
        }
        let file = self.writer.as_mut().expect("opened above");
        let mut lines = String::new();
        for row in rows {
            lines.push_str(&serde_json::to_string(row).map_err(json_error)?);
            lines.push('\n');
        }
        file.write_all(lines.as_bytes())?;

        file.sync_data()
    }

    fn close(&mut self) -> io::Result<()> {
        let Some(file) = self.writer.take() else {
            return Ok(());
        };

        file.sync_all()
    }
}

/* === Public Functions === */
pub fn read_rows<S: DeserializeOwned>(path: impl AsRef<Path>) -> io::Result<Vec<S>> {
    let mut rows = Vec::new();
    for line in BufReader::new(File::open(path)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str(&line).map_err(json_error)?);
    }

    Ok(rows)
}

/* === Private Functions === */
fn encode<A: Application, S: Stream<A>>(batch: &Batch<A>) -> io::Result<Vec<Map<String, Value>>> {
    let mut rows = Vec::new();
    for row in batch.select::<S>() {
        let Value::Object(mut members) = serde_json::to_value(&row.value).map_err(json_error)?
        else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a stream row does not serialize as a JSON object",
            ));
        };
        if members.contains_key(POSITION) || members.contains_key(GAP) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a stream row uses a JSON Lines technical member name",
            ));
        }
        members.insert(POSITION.to_owned(), Value::from(row.position.0));
        // Present on every line, and null unless rows were lost before it: a
        // reader's `Option<u64>` never meets a missing member.
        members.insert(GAP.to_owned(), Value::Null);
        rows.push(members);
    }

    Ok(rows)
}

fn heal(directory: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(SUFFIX))
        {
            truncate_partial(&path)?;
        }
    }

    Ok(())
}

fn truncate_partial(path: &Path) -> io::Result<()> {
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    let length = file.metadata()?.len();
    if length == 0 {
        return Ok(());
    }
    let mut window = vec![0; WINDOW];
    let mut end = length;
    while end > 0 {
        let start = end.saturating_sub(WINDOW as u64);
        let read = (end - start) as usize;
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut window[..read])?;
        if let Some(at) = window[..read].iter().rposition(|byte| *byte == b'\n') {
            let complete = start + at as u64 + 1;
            if complete < length {
                file.set_len(complete)?;
                file.sync_all()?;
            }
            return Ok(());
        }
        end = start;
    }
    // Not one complete line anywhere: nothing here was ever readable.
    file.set_len(0)?;

    file.sync_all()
}

fn read_checkpoint(directory: &Path) -> io::Result<Cursor> {
    let path = directory.join(CHECKPOINT);
    if !path.exists() {
        return Ok(Cursor::ORIGIN);
    }
    let mut stored = String::new();
    File::open(path)?.read_to_string(&mut stored)?;

    stored
        .trim()
        .parse::<u64>()
        .map(Cursor)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn persist_checkpoint(directory: &Path, cursor: Cursor) -> io::Result<()> {
    let temporary = directory.join(CHECKPOINT_TEMP);
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)?;
    writeln!(file, "{}", cursor.0)?;
    file.sync_all()?;
    std::fs::rename(temporary, directory.join(CHECKPOINT))?;
    // Directory fsync is a POSIX idiom, as it is for the journal and the
    // evidence directory beside this module: opening a directory as a file is
    // refused on Windows (`ERROR_ACCESS_DENIED`), and NTFS makes rename
    // metadata durable without a user-mode directory flush.
    if cfg!(unix) {
        File::open(directory)?.sync_all()?;
    }

    Ok(())
}

fn capture_path(directory: &Path, id: &str) -> PathBuf {
    directory.join(format!("{id}{SUFFIX}"))
}

fn json_error(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

/* === Tests === */
