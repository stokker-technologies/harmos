/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::access::GrantAccess;
use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_protocol::storage::StorageFailureKind;
use harmos_storage::query::columnar::{Cancellation, ColumnKind, ColumnSource};
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

pub(super) fn scope(source: &ColumnSource) -> String {
    let kind = match source.kind {
        ColumnKind::Objects => "objects",
        ColumnKind::Records => "records",
        ColumnKind::Stream => "streams",
    };
    format!("{kind}:schema:{}", source.schema.id)
}
pub(super) fn grant(
    id: &'static str, source: Option<harmos_protocol::storage::ColumnSource>,
) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
    let source: ColumnSource = harmos_storage::query::protobuf::required(source)
        .and_then(TryInto::try_into)
        .map_err(|e| harmos_domain::MessageError::new(e.to_string()))?;
    Ok(Some(crate::plugin::Grant::of(id, scope(&source))))
}
pub(super) fn admit(context: &ExecutionContext, id: &'static str, source: &ColumnSource) -> crate::Result<()> {
    let grants = context.resource::<GrantAccess>().map_err(|_| crate::Error::InvalidInput {
        field: "storage grants",
        reason: "missing authority",
    })?;
    grants.0.admit(&crate::plugin::Grant::of(id, scope(source))).map_err(crate::Error::Access)
}
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
            harmos_storage::Error::IndexNotFound { .. } => StorageFailureKind::StorageFailureMissingIndex,
            harmos_storage::Error::NotReady { domain: "expired storage snapshot" } => StorageFailureKind::StorageFailureExpired,
            harmos_storage::Error::NotReady { .. } => StorageFailureKind::StorageFailureNotReady,
            harmos_storage::Error::Gap { .. } => StorageFailureKind::StorageFailureGap,
            harmos_storage::Error::ResourceExhausted { .. } => StorageFailureKind::StorageFailureExhausted,
            harmos_storage::Error::InvalidInput {
                field: "saved snapshot" | "storage snapshot" | "storage continuation",
                ..
            } => StorageFailureKind::StorageFailureExpired,
            _ => match source.backend_failure() {
                Some(harmos_storage::BackendFailure::Unsupported) => StorageFailureKind::StorageFailureUnsupported,
                Some(harmos_storage::BackendFailure::Conflict) => StorageFailureKind::StorageFailureConflict,
                _ => StorageFailureKind::StorageFailureInvalid,
            },
        },
        _ => StorageFailureKind::StorageFailureInvalid,
    };
    let failure = crate::Failure::new(super::classify(&error), error.to_string()).with_detail("storage.kind", kind.as_str_name());
    ServingError::Handler(failure)
}

pub(super) fn page_bytes(bytes: usize, limit: usize) -> crate::Result<()> {
    if bytes > limit {
        return Err(harmos_storage::Error::ResourceExhausted { resource: "storage wire page" }.into());
    }
    Ok(())
}
