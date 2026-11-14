/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::{ExecutionContext, ServingError};

use harmos_storage::query::columnar::{Cancellation, ColumnSource};
use std::future::Future;

/* === Structs === */

struct CancelOnDrop(Cancellation);

/* === Implementations === */

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/* === Public Functions === */

pub(super) fn source_matches(expected: &ColumnSource, actual: &ColumnSource) -> crate::Result<()> {
    if expected != actual {
        return Err(crate::Error::InvalidInput {
            field: "storage source",
            reason: "source differs from the registered definition",
        });
    }
    Ok(())
}
pub(super) async fn run<T>(
    context: &ExecutionContext, work: impl Future<Output = crate::Result<T>>, cancel: Cancellation,
) -> Result<T, ServingError> {
    let _guard = CancelOnDrop(cancel);
    tokio::select! {
        biased;
        _ = context.cancellation.cancelled() => Err(ServingError::Closing),
        result = work => result.map_err(failure),
    }
}
pub(super) fn failure(error: crate::Error) -> ServingError {
    let kind = match &error {
        crate::Error::Storage(source) => match source {
            harmos_storage::Error::IndexNotFound { .. } => "STORAGE_FAILURE_MISSING_INDEX",
            harmos_storage::Error::NotReady { domain: "expired storage snapshot" } => "STORAGE_FAILURE_EXPIRED",
            harmos_storage::Error::NotReady { .. } => "STORAGE_FAILURE_NOT_READY",
            harmos_storage::Error::Gap { .. } => "STORAGE_FAILURE_GAP",
            harmos_storage::Error::ResourceExhausted { .. } => "STORAGE_FAILURE_EXHAUSTED",
            harmos_storage::Error::InvalidInput {
                field: "saved snapshot" | "storage snapshot" | "storage continuation",
                ..
            } => "STORAGE_FAILURE_EXPIRED",
            _ => match source.backend_failure() {
                Some(harmos_storage::BackendFailure::Unsupported) => "STORAGE_FAILURE_UNSUPPORTED",
                Some(harmos_storage::BackendFailure::Conflict) => "STORAGE_FAILURE_CONFLICT",
                _ => "STORAGE_FAILURE_INVALID",
            },
        },
        _ => "STORAGE_FAILURE_INVALID",
    };
    let failure = crate::Failure::new(super::classify(&error), error.to_string()).with_detail("storage.kind", kind);
    ServingError::Handler(failure)
}

pub(super) fn page_bytes(bytes: usize, limit: usize) -> crate::Result<()> {
    if bytes > limit {
        return Err(harmos_storage::Error::ResourceExhausted { resource: "storage response page" }.into());
    }
    Ok(())
}

pub(super) fn read_response<T: crate::MemorySize>(context: &ExecutionContext, response: T) -> Result<T, ServingError> {
    let storage = context.resource::<harmos_storage::StorageManager>().map_err(|_| ServingError::Catalog)?;
    page_bytes(response.memory_size(), storage.settings().response.batch_bytes.get()).map_err(failure)?;
    Ok(response)
}

pub(super) fn plan(value: &impl serde::Serialize) -> crate::Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|error| crate::Error::Access(Box::new(error)))
}
