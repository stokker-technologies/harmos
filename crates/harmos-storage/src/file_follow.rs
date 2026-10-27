/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Follow Immutable Storage Or A Local Path With Explicit Identity Changes.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::PathBuf;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::local::LocalReader;
use crate::manager::Inner;
use crate::{ByteRange, Error, FileBatch, FileCompletion, FileReadEvent, FileReader, Result, StorageManager};
use harmos_domain::model::storage::files::{FileEvent, FileSource, FollowStart};
use harmos_domain::operations::storage::{FollowFile, ReadFile};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct FileFollower {
    context: crate::execution::Context,
    inner: Arc<Inner>,
    source: Source,

    delivered: u64,
    finished: bool,
}

struct Local {
    path: PathBuf,
    reader: Option<LocalReader>,
    missing: bool,
}

/* === Enums === */

enum Source {
    Stored(FileReader),
    Local(Local),
}

enum Step {
    Data(FileBatch),
    Signal(FileEvent),
    Wait,
    Done,
}

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.file_follow.follow_file", level = "debug", skip_all)]
    pub async fn follow_file(&self, request: FollowFile) -> Result<FileFollower> {
        self.open_follower(request).await
    }

    pub(crate) async fn open_follower(&self, request: FollowFile) -> Result<FileFollower> {
        let context = crate::execution::Context::current();
        let span = context.sync(|| tracing::debug_span!("storage.file.follow_file", outcome = "cancelled"));
        context
            .result(span, async {
                let source = match request.source {
                    FileSource::Local(path) => FileSource::Local(std::path::absolute(path).map_err(crate::local::io_error)?),
                    stored => stored,
                };

                let source = match source {
                    FileSource::Stored(file) => {
                        let offset = start(request.start, file.length);
                        let reader =
                            self.open_file(ReadFile { source: FileSource::Stored(file), range: ByteRange { offset, limit: None } }).await?;
                        Source::Stored(reader)
                    }
                    FileSource::Local(path) => {
                        let mut reader = LocalReader::open(&self.inner, path.clone(), ByteRange { offset: 0, limit: None }).await?;
                        reader.start(start(request.start, reader.length()))?;
                        Source::Local(Local { path, reader: Some(reader), missing: false })
                    }
                };
                Ok(FileFollower {
                    context: crate::execution::Context::current(),
                    inner: Arc::clone(&self.inner),
                    source,

                    delivered: 0,
                    finished: false,
                })
            })
            .await
    }
}

impl FileFollower {
    pub async fn next(&mut self) -> Result<Option<FileReadEvent>> {
        let context = self.context.clone();
        let span = context.sync(|| tracing::trace_span!("storage.reader.page", outcome = "cancelled"));
        let future = async {
            loop {
                if self.finished {
                    return Ok(None);
                }
                if self.inner.state.lock().unwrap().status.closing {
                    return Err(Error::Closing { domain: "file follower" });
                }
                let step = match &mut self.source {
                    Source::Stored(reader) => match reader.next_raw(true).await? {
                        Some(batch) => Step::Data(batch),
                        None => Step::Done,
                    },
                    Source::Local(local) => local.next(&self.inner).await?,
                };
                match step {
                    Step::Data(batch) => {
                        self.delivered += batch.bytes.len() as u64;
                        return Ok(Some(FileReadEvent::new(FileEvent::Data(batch))));
                    }
                    Step::Signal(event) => {
                        return Ok(Some(FileReadEvent::new(event)));
                    }
                    Step::Done => {
                        self.finished = true;
                        return Ok(None);
                    }
                    Step::Wait => {
                        let mut changed = self.inner.changed.subscribe();
                        tokio::select! {
                            _ = tokio::time::sleep(self.inner.settings.filesystem.poll_interval) => {},
                            _ = changed.changed() => {},
                        }
                    }
                }
            }
        };
        context.result(span, future).await
    }

    pub fn completion(&self) -> Result<FileCompletion> {
        if !self.finished {
            return Err(Error::NotReady { domain: "file follow completion" });
        }
        Ok(FileCompletion { bytes: self.delivered })
    }
}

impl Local {
    async fn next(&mut self, inner: &Arc<Inner>) -> Result<Step> {
        if self.reader.is_none() {
            self.reader = Some(LocalReader::open(inner, self.path.clone(), ByteRange { offset: 0, limit: None }).await?);
        }
        let reader = self.reader.as_ref().expect("local follower retains its current file");
        let Some((same, length)) = reader.inspect().await? else {
            if self.missing {
                return Ok(Step::Wait);
            }
            self.missing = true;
            return Ok(Step::Signal(FileEvent::Replaced));
        };
        if !same {
            self.reader.take();
            self.reader = Some(LocalReader::open(inner, self.path.clone(), ByteRange { offset: 0, limit: None }).await?);
            let signaled = std::mem::replace(&mut self.missing, false);
            return Ok(if signaled { Step::Wait } else { Step::Signal(FileEvent::Replaced) });
        }
        let reader = self.reader.as_mut().expect("local follower retains its current file");
        self.missing = false;
        if length < reader.offset() {
            self.reader.take();
            self.reader = Some(LocalReader::open(inner, self.path.clone(), ByteRange { offset: 0, limit: None }).await?);
            return Ok(Step::Signal(FileEvent::Truncated { length }));
        }
        reader.retarget(length);
        match reader.next(true).await? {
            Some(batch) => Ok(Step::Data(batch)),
            None => Ok(Step::Wait),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn start(start: FollowStart, length: u64) -> u64 {
    match start {
        FollowStart::Beginning => 0,
        FollowStart::End => length,
        FollowStart::At(offset) => offset,
    }
}

/* === Tests === */
