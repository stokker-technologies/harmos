/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared File, Production And Directory Observation

/* === Imports === */

use super::requests::{DirectoryEvent, DirectorySelection, FileBatch, FileCompletion, FileEvent, FileProgress, FollowStart, StagedFile};

use super::requests::required;

use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_storage::{FileSource, StorageManager};
use std::{future::Future, num::NonZeroUsize};

/* === Public Functions === */

pub(super) async fn bytes<F, Fut>(
    context: &ExecutionContext, source: Option<FileSource>, start: Option<FollowStart>, mut emit: F,
) -> Result<FileCompletion, ServingError>
where
    F: FnMut(FileEvent) -> Fut,
    Fut: Future<Output = Result<(), ServingError>>,
{
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;

    let work: Result<_, crate::Error> = async {
        let source: FileSource = required(source)?;
        if let FileSource::Stored(file) = &source
            && !storage.get_saved_position().covers(&file.position)?
        {
            return Err(harmos_storage::Error::NotReady { domain: "WatchFileBytes durable content" }.into());
        }
        storage.follow_file(harmos_storage::FileFollow { source, start: required(start)? }).await.map_err(Into::into)
    }
    .await;
    let mut reader = work.map_err(super::admission::failure)?;
    loop {
        let next = tokio::select! {
            biased;
            _ = context.cancellation.cancelled() => return Err(ServingError::Closing),
            next = reader.next() => next.map_err(|error| super::admission::failure(error.into()))?,
        };
        let Some(event) = next else {
            return reader.completion().map(Into::into).map_err(|error| super::admission::failure(error.into()));
        };
        let event: FileEvent = (*event).clone().into();
        match event {
            FileEvent::Data(batch) => {
                let mut start = 0;
                while start < batch.bytes.len() {
                    let offset = batch.offset + start as u64;
                    let count = capacity(storage.settings().response.batch_bytes.get(), offset, batch.bytes.len() - start, 2)
                        .map_err(|error| super::admission::failure(error.into()))?;
                    emit(FileEvent::Data(FileBatch {
                        offset,
                        bytes: batch.bytes[start..start + count].to_vec(),
                    }))
                    .await?;
                    start += count;
                }
            }
            event => emit(event).await?,
        }
    }
}

pub(super) async fn production<F, Fut>(
    context: &ExecutionContext, file: Option<StagedFile>, offset: u64, mut emit: F,
) -> Result<FileProgress, ServingError>
where
    F: FnMut(FileBatch) -> Fut,
    Fut: Future<Output = Result<(), ServingError>>,
{
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let file: harmos_storage::StagedFile = required(file)?;

        Ok((storage.follow_production(&file, offset).await?, file.id))
    }
    .await;
    let (mut reader, _file) = result.map_err(super::admission::failure)?;
    let mut next_offset = offset;
    loop {
        let maximum = NonZeroUsize::new(
            capacity(storage.settings().response.batch_bytes.get(), next_offset, storage.settings().response.batch_bytes.get(), 1)
                .map_err(|error| super::admission::failure(error.into()))?,
        )
        .unwrap();
        let next = tokio::select! {
            biased;
            _ = context.cancellation.cancelled() => return Err(ServingError::Closing),
            next = reader.next_batch(maximum) => next.map_err(|error| super::admission::failure(error.into()))?,
        };
        let Some(batch) = next else {
            return Ok(reader.progress().into());
        };

        next_offset = batch.offset() + batch.bytes().len() as u64;
        emit(FileBatch {
            offset: batch.offset(),
            bytes: batch.bytes().to_vec(),
        })
        .await?;
    }
}

pub(super) async fn directory<F, Fut>(
    context: &ExecutionContext, selection: Option<DirectorySelection>, include_existing: bool, mut emit: F,
) -> Result<(), ServingError>
where
    F: FnMut(DirectoryEvent) -> Fut,
    Fut: Future<Output = Result<(), ServingError>>,
{
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async { Ok(storage.watch_directory(required(selection)?, include_existing).await?) }.await;
    let mut reader = result.map_err(super::admission::failure)?;
    loop {
        let next = tokio::select! {
            biased;
            _ = context.cancellation.cancelled() => return Err(ServingError::Closing),
            next = reader.next() => next.map_err(|error| super::admission::failure(error.into()))?,
        };
        let Some(event) = next else {
            return Ok(());
        };
        emit((*event).clone().into()).await?;
    }
}

pub(super) async fn send<T: crate::MemorySize + Send + Sync + 'static>(
    context: &ExecutionContext, id: &'static str, output: &super::NativeOutput<T>, response: T,
) -> Result<(), ServingError> {
    let response = super::admission::read_response(context, response)?;
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    storage
        .clock()
        .timeout(storage.settings().policy.io_timeout, output.send(response))
        .await
        .map_err(|_| {
            super::admission::failure(
                harmos_storage::Error::backend(
                    id,
                    harmos_storage::BackendFailure::Transient,
                    std::io::Error::new(std::io::ErrorKind::TimedOut, "consumer delivery deadline exceeded"),
                )
                .into(),
            )
        })?
        .map_err(ServingError::Stream)
}

/* === Private Functions === */

// Count Full Nested Protobuf Frames Without Allocating Candidate Payloads.
fn capacity(ceiling: usize, offset: u64, remaining: usize, wrappers: usize) -> harmos_storage::Result<usize> {
    let mut lower = 0;
    let mut upper = remaining.min(ceiling);
    while lower < upper {
        let count = lower + (upper - lower).div_ceil(2);
        let mut length = if offset == 0 { 0 } else { 1 + prost::encoding::encoded_len_varint(offset) };
        length += 1 + prost::length_delimiter_len(count) + count;
        for _ in 0..wrappers {
            length = 1 + prost::length_delimiter_len(length) + length;
        }
        if length <= ceiling {
            lower = count;
        } else {
            upper = count - 1;
        }
    }
    if lower == 0 {
        return Err(harmos_storage::Error::ResourceExhausted {
            resource: "file observation response framing",
        });
    }
    Ok(lower)
}
