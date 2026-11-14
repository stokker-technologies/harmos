/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Durable Row Observation Handler

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::requests::required;

use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_protocol::v1;
use harmos_storage::follow::Follower;
use harmos_storage::query::Source;
use harmos_storage::{ObjectEntry, StorageCursor, StorageManager, StoredRecord, StoredRow};
use std::future::Future;

/* === Structs === */

#[derive(Clone)]
pub(super) struct Frame {
    pub(super) at: v1::ReadCut,
    pub(super) event: Event,
}

/* === Enums === */

#[derive(Clone)]
pub(super) enum Event {
    Boundary,
    Record(StoredRecord),
    Sample(StoredRow),
    Object(ObjectEntry),
    Progress,
    Resync(String),
}
enum Rows {
    Records(Follower<StoredRecord>),
    Samples(Follower<StoredRow>),
    Objects(Follower<ObjectEntry>),
}

/* === Implementations === */

impl Rows {
    async fn next(&mut self) -> harmos_storage::Result<(Vec<(Event, StorageCursor)>, StorageCursor)> {
        match self {
            Self::Records(rows) => {
                let (page, through) = rows.next_durable().await?;
                Ok((page.entries().iter().map(|row| (Event::Record(row.clone().into()), row.position.clone())).collect(), through))
            }
            Self::Samples(rows) => {
                let (page, through) = rows.next_durable().await?;
                Ok((page.entries().iter().map(|row| (Event::Sample(row.clone().into()), row.record.position.clone())).collect(), through))
            }
            Self::Objects(rows) => {
                let (page, through) = rows.next_durable().await?;
                Ok((page.entries().iter().map(|row| (Event::Object(row.clone().into()), row.position.clone())).collect(), through))
            }
        }
    }
}

/* === Public Functions === */

pub(super) async fn run<F, Fut>(
    context: &ExecutionContext, source: Option<Source>, at: Option<v1::ReadCut>, mut emit: F,
) -> Result<Frame, ServingError>
where
    F: FnMut(Frame) -> Fut,
    Fut: Future<Output = Result<(), ServingError>>,
{
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;

    let selected: Result<_, crate::Error> = async {
        let source: Source = required(source)?;
        source.validate()?;

        let read = snapshots.resolve(&storage, at).await?;
        let rows = match source {
            Source::Records(source) => Rows::Records(storage.follow::<StoredRecord>(Some(read.at()), source).await?),
            Source::Stream(source) if source.after == 0 => Rows::Samples(storage.follow::<StoredRow>(Some(read.at()), source).await?),
            Source::Objects(source) if source.follows_revisions() => {
                Rows::Objects(storage.follow::<ObjectEntry>(Some(read.at()), source).await?)
            }
            _ => {
                return Err(harmos_storage::Error::InvalidInput {
                    field: "WatchRows",
                    reason: "select records, samples without an alternate position, or object revisions",
                }
                .into());
            }
        };
        Ok((read, rows))
    }
    .await;
    let (read, mut rows) = selected.map_err(super::admission::failure)?;
    let mut last = read.cut().clone();
    emit(Frame { at: last.clone(), event: Event::Boundary }).await?;
    // The Follower Has Registered Its Own Pinned History And Queue Retention.
    drop(read);
    loop {
        let next = tokio::select! {
            biased;
            _ = context.cancellation.cancelled() => return Err(ServingError::Closing),
            next = rows.next() => next,
        };
        let (entries, through) = match next {
            Ok(next) => next,
            Err(error @ harmos_storage::Error::Gap { .. }) => {
                let frame = Frame {
                    at: last,
                    event: Event::Resync(format!("WatchRows: {error}; list a fresh image and reopen from its issued cut")),
                };
                emit(frame.clone()).await?;
                return Ok(frame);
            }
            Err(error) => return Err(super::admission::failure(error.into())),
        };

        for (event, position) in entries {
            last = snapshots.composite(&storage, super::transfers::cut(position)).await.map_err(super::admission::failure)?;
            emit(Frame { at: last.clone(), event }).await?;
        }
        let progress = super::transfers::cut(through);
        if progress != last {
            last = snapshots.composite(&storage, progress).await.map_err(super::admission::failure)?;
            emit(Frame { at: last.clone(), event: Event::Progress }).await?;
        }
    }
}
