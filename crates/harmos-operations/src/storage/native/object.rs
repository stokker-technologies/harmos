/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Object Get And Set Handlers

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::requests::{MutationId, ObjectValue, SchemaInput, SchemaKey};

use super::requests::required;

use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_protocol::v1;
use harmos_storage::StorageManager;
use harmos_query::columnar::ColumnKind;
use harmos_query::columnar::ColumnSource;
use prost::Message;

pub(super) enum SetOutcome {
    Applied(Box<ObjectValue>, Vec<u8>, v1::Receipt),
    Conflict(Vec<u8>),
}

/* === Public Functions === */

pub(super) fn revision(value: harmos_storage::ObjectVersionToken) -> Vec<u8> {
    v1::Position {
        history: Some(value.history().clone().into()),
        sequence: value.revision().get(),
    }
    .encode_to_vec()
}
pub(super) async fn get(
    context: &ExecutionContext, schema: Option<SchemaKey>, item: String, at: Option<v1::ReadCut>,
) -> Result<(Option<ObjectValue>, Vec<u8>, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let schema = required(schema)?;
        let source = ColumnSource { kind: ColumnKind::Objects, schema };

        let read = snapshots.resolve(&storage, at).await?;
        let point = harmos_storage::ObjectPoint {
            schema: source.schema.id,
            item: harmos_storage::ObjectKey::from_owned(item)?,
            version: harmos_storage::ObjectVersion::Latest,
        };
        let selected = read.get_object(&point).await?;
        let (value, revision) = selected.map_or((None, vec![]), |entry| (Some(entry.value), revision(entry.version)));
        Ok((value, revision, read.cut().clone()))
    }
    .await;
    result.map_err(super::admission::failure)
}
pub(super) async fn set(
    context: &ExecutionContext, mutation: Option<MutationId>, expected_revision: Vec<u8>, create_if_absent: bool,
    schema: Option<SchemaInput>, value: Option<ObjectValue>,
) -> Result<SetOutcome, ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let value: harmos_storage::ObjectValue = required(value)?;
        let _source = ColumnSource {
            kind: ColumnKind::Objects,
            schema: value.schema.clone(),
        };

        let schema = required(schema)?;
        let key: harmos_storage::SchemaKey = required(schema.key)?;
        if key != value.schema {
            return Err(crate::Error::InvalidInput {
                field: "SetObject",
                reason: "schema differs from the object value",
            });
        }
        let graph = required(schema.graph)?;
        let expected = if create_if_absent && expected_revision.is_empty() {
            None
        } else if !create_if_absent && !expected_revision.is_empty() {
            let encoded = v1::Position::decode(expected_revision.as_slice()).map_err(|_| crate::Error::InvalidInput {
                field: "SetObject",
                reason: "invalid expected revision; get again and retry",
            })?;
            let position: harmos_storage::StorageCursor = encoded.try_into()?;
            if position.sequence == 0 {
                return Err(crate::Error::InvalidInput {
                    field: "SetObject",
                    reason: "zero object revision",
                });
            }
            let expected = harmos_storage::ObjectVersionToken::new(position);
            if revision(expected.clone()) != expected_revision {
                return Err(crate::Error::InvalidInput {
                    field: "SetObject",
                    reason: "noncanonical expected revision; get again and retry",
                });
            }
            Some(expected)
        } else {
            return Err(crate::Error::InvalidInput {
                field: "SetObject",
                reason: "supply the Get revision or explicit create-if-absent",
            });
        };
        let mutation = required(mutation)?;
        let identity = mutation.clone();
        let outcome = storage.compare_exchange_value_once(&identity, expected, value.clone(), graph).await?;
        Ok(match outcome {
            harmos_storage::CompareExchange::Applied(applied) => SetOutcome::Applied(
                Box::new(value),
                revision(applied.version),
                super::transfers::receipt(&mutation, Some(applied.position)),
            ),
            harmos_storage::CompareExchange::Conflict { current } => SetOutcome::Conflict(current.map(revision).unwrap_or_default()),
        })
    }
    .await;
    result.map_err(super::admission::failure)
}
