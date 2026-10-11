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
use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
// Third Party Imports

// Local Imports
use crate::{At, Cursor, Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy, Debug)]
pub struct ReadLimits {
    pub rows: usize,
    pub row_bytes: usize,
    pub batch_bytes: usize,
    pub batch_rows: usize,
    pub segments: usize,
    pub segment_bytes: usize,
    pub decoded_bytes: usize,
    pub pinned_bytes: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct PageLimits {
    pub max_rows: usize,
    pub max_encoded_bytes: usize,
}

#[derive(Debug)]
pub struct RowPage<S> {
    pub rows: Vec<At<Arc<S>>>,
    pub encoded_bytes: usize,
    pub complete: bool,
}

struct Ready<S> {
    row: At<Arc<S>>,
    encoded_bytes: usize,
}

struct Owned {
    source: Box<dyn RowSource>,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

pub struct RowReader<S> {
    owned: Option<Owned>,
    cut: ReadCut,
    limits: ReadLimits,
    returned: usize,
    done: bool,
    partition: Option<super::Partition<S>>,
    pending: Option<Ready<S>>,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadCut {
    Window {
        delivered: Cursor,
    },
    Series {
        epoch: u64,
        delivered: Cursor,
        ordinal: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    #[error("the active row reader budget is exhausted")]
    Busy,
    #[error("row read exceeds its {0} limit")]
    Limit(&'static str),
    #[error("the attached series adapter does not support bounded reads")]
    Unsupported,
    #[error("bounded row decoding does not support {0}")]
    EncodingUnsupported(&'static str),
    #[error("no series adapter is attached")]
    Unserved,
    #[error("row history is retained only above {floor:?}")]
    Retained { floor: Position },
    #[error("selected stream capture is incomplete or uncertified")]
    IncompleteCapture,
    #[error("row source is unreadable: {0}")]
    Unreadable(String),
    #[error("row reader is closed")]
    Closed,
}

/* === Traits === */
pub trait RowSource: Send {
    fn cut(&self) -> ReadCut;

    #[allow(clippy::type_complexity)]
    fn next(
        &mut self,
        cancelled: &AtomicBool,
    ) -> Result<Option<At<Arc<dyn Any + Send + Sync>>>, ReadError>;
}

/* === Implementations === */
impl Default for ReadLimits {
    fn default() -> Self {
        Self {
            rows: 100_000,
            row_bytes: 1024 * 1024,
            batch_bytes: 8 * 1024 * 1024,
            batch_rows: 65_536,
            segments: 256,
            segment_bytes: 64 * 1024 * 1024,
            decoded_bytes: 32 * 1024 * 1024,
            pinned_bytes: 16 * 1024 * 1024,
        }
    }
}

impl Default for PageLimits {
    fn default() -> Self {
        Self {
            max_rows: 256,
            max_encoded_bytes: 256 * 1024,
        }
    }
}

impl<S: serde::Serialize + Send + Sync + 'static> RowReader<S> {
    pub fn cut(&self) -> ReadCut {
        self.cut
    }

    pub async fn next_page(&mut self, page_limits: PageLimits) -> Result<RowPage<S>, ReadError> {
        if page_limits.max_rows == 0 || page_limits.max_encoded_bytes == 0 {
            return Err(ReadError::Limit("positive page limits"));
        }
        let Some(mut owned) = self.owned.take() else {
            return if self.done {
                Ok(RowPage {
                    rows: Vec::new(),
                    encoded_bytes: 0,
                    complete: true,
                })
            } else {
                Err(ReadError::Closed)
            };
        };
        let partition = self.partition.take();
        let mut pending = self.pending.take();
        let returned = self.returned;
        let limits = self.limits;
        struct Cancel(Arc<AtomicBool>);
        impl Drop for Cancel {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let _cancel = Cancel(cancelled.clone());
        let result = tokio::task::spawn_blocking(move || {
            let mut page = RowPage {
                rows: Vec::new(),
                encoded_bytes: 0usize,
                complete: false,
            };
            loop {
                if cancelled.load(Ordering::Acquire) {
                    return Err(ReadError::Closed);
                }
                let next = match pending.take() {
                    Some(ready) => Some(ready),
                    None => loop {
                        if cancelled.load(Ordering::Acquire) {
                            return Err(ReadError::Closed);
                        }
                        let Some(row) = owned.source.next(&cancelled)? else {
                            break None;
                        };
                        let typed = super::row_value::<S>(row.value.as_ref()).ok_or_else(|| {
                            ReadError::Unreadable("source row type mismatch".into())
                        })?;
                        if partition.as_ref().is_some_and(|p| !(p.matches)(typed)) {
                            continue;
                        }
                        // Validate the borrowed original before allocating a legacy raw copy.
                        let encoded_bytes = encoded_size(typed, limits.row_bytes)?;
                        let row_value = if let Some(managed) =
                            row.value.downcast_ref::<super::ManagedPublication>()
                        {
                            Arc::new(managed.row().clone()) as Arc<dyn std::any::Any + Send + Sync>
                        } else {
                            row.value
                        };
                        let value = row_value.downcast::<S>().map_err(|_| {
                            ReadError::Unreadable("source row type mismatch".into())
                        })?;
                        break Some(Ready {
                            row: At {
                                value,
                                position: row.position,
                            },
                            encoded_bytes,
                        });
                    },
                };
                let Some(ready) = next else {
                    page.complete = true;
                    break;
                };
                if returned + page.rows.len() == limits.rows {
                    return Err(ReadError::Limit("rows"));
                }
                let bytes = page.encoded_bytes.checked_add(ready.encoded_bytes);
                if page.rows.len() == page_limits.max_rows
                    || bytes.is_none_or(|bytes| bytes > page_limits.max_encoded_bytes)
                {
                    if page.rows.is_empty() {
                        return Err(ReadError::Limit("page row bytes"));
                    }
                    // One row of lookahead makes terminal completion exact,
                    // even at a full-page boundary. Acquisition reserves this
                    // slot from pinned_bytes; its encoding is measured once.
                    pending = Some(ready);
                    break;
                }
                page.encoded_bytes = bytes.expect("within the checked page budget");
                page.rows.push(ready.row);
            }
            Ok((owned, partition, pending, page))
        })
        .await
        .map_err(|_| ReadError::Closed)?;
        let (owned, partition, pending, page) = result?;
        self.partition = partition;
        self.returned += page.rows.len();
        self.done = page.complete;
        if !page.complete {
            self.owned = Some(owned);
            self.pending = pending;
        }
        Ok(page)
    }
}

impl<S: Clone + serde::Serialize + Send + Sync + 'static> super::Query<S> {
    pub async fn reader(self, limits: ReadLimits) -> Result<RowReader<S>, ReadError> {
        if [
            limits.rows,
            limits.row_bytes,
            limits.batch_bytes,
            limits.batch_rows,
            limits.segments,
            limits.segment_bytes,
            limits.decoded_bytes,
            limits.pinned_bytes,
        ]
        .contains(&0)
        {
            return Err(ReadError::Limit("positive limits"));
        }
        let reserved = limits
            .row_bytes
            .checked_add(std::mem::size_of::<Ready<S>>())
            .ok_or(ReadError::Limit("pending row reservation"))?;
        let mut source_limits = limits;
        source_limits.pinned_bytes = limits
            .pinned_bytes
            .checked_sub(reserved)
            .ok_or(ReadError::Limit("pending row reservation"))?;
        let permit = self.tier.reader_permit()?;
        let end = match self.as_of {
            Some(cut) => match self.end {
                std::ops::Bound::Unbounded => std::ops::Bound::Included(cut),
                std::ops::Bound::Included(end) if end > cut => std::ops::Bound::Included(cut),
                std::ops::Bound::Excluded(end) if end > cut => std::ops::Bound::Included(cut),
                end => end,
            },
            None => self.end,
        };
        let label_bytes = self.partition.as_ref().map_or(Some(0usize), |partition| {
            partition
                .labels
                .iter()
                .try_fold(0usize, |total, label| total.checked_add(label.len() + 32))
        });
        if label_bytes.is_none_or(|bytes| bytes > limits.pinned_bytes) {
            return Err(ReadError::Limit("track bytes"));
        }
        let track = self
            .partition
            .as_ref()
            .map(|p| p.labels.clone())
            .unwrap_or_default();
        let opened = self
            .tier
            .reader(
                self.definition,
                track,
                self.start,
                end,
                source_limits,
                permit,
            )
            .await?;
        let source = match opened.source {
            Err(ReadError::Unserved) => self.window.reader::<S>(self.start, end, source_limits)?,
            result => result?,
        };
        Ok(RowReader {
            cut: source.cut(),
            owned: Some(Owned {
                source,
                _permit: opened.permit,
            }),
            limits,
            returned: 0,
            done: false,
            partition: self.partition,
            pending: None,
        })
    }
}

/* === Public Functions === */
pub(crate) fn encoded_size<S: serde::Serialize>(row: &S, limit: usize) -> Result<usize, ReadError> {
    struct Count {
        size: usize,
        limit: usize,
    }
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.size = self
                .size
                .checked_add(bytes.len())
                .filter(|size| *size <= self.limit)
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::FileTooLarge, "row byte limit")
                })?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count { size: 0, limit };
    serde_json::to_writer(&mut count, row).map_err(|error| {
        if error.io_error_kind() == Some(std::io::ErrorKind::FileTooLarge) {
            ReadError::Limit("row bytes")
        } else {
            ReadError::Unreadable(error.to_string())
        }
    })?;
    Ok(count.size)
}

/* === Private Functions === */

/* === Tests === */
