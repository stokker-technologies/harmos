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

use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_protocol::{storage as wire, v1};
use harmos_storage::{
    StorageManager,
    query::{
        columnar::{ColumnKind, ColumnSource},
        protobuf::required,
    },
};
use prost::Message;

pub(super) enum SetOutcome {
    Applied(wire::ObjectValue, Vec<u8>, Box<v1::Receipt>),
    Conflict(Vec<u8>),
}

/* === Public Functions === */

pub(super) fn source(schema: Option<wire::SchemaKey>) -> Option<wire::ColumnSource> {
    schema.map(|schema| wire::ColumnSource {
        kind: wire::ColumnKind::Objects as i32,
        schema: Some(schema),
    })
}
pub(super) fn revision(value: harmos_storage::ObjectVersionToken) -> Vec<u8> {
    let value: wire::ObjectVersionToken = value.into();
    value.encode_to_vec()
}
pub(super) async fn get(
    context: &ExecutionContext, id: &'static str, schema: Option<wire::SchemaKey>, item: String, at: Option<v1::ReadCut>,
) -> Result<(Option<wire::ObjectValue>, Vec<u8>, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let schema = required(schema)?.try_into()?;
        let source = ColumnSource { kind: ColumnKind::Objects, schema };
        super::native::admit(context, id, &source)?;
        let read = snapshots.resolve(&storage, at).await?;
        let point = harmos_storage::ObjectPoint {
            schema: source.schema.id,
            item: harmos_storage::ObjectKey::from_owned(item)?,
            version: harmos_storage::ObjectVersion::Latest,
        };
        let selected = read.get_object(&point).await?;
        let (value, revision) = selected.map_or((None, vec![]), |entry| (Some(entry.value.into()), revision(entry.version)));
        Ok((value, revision, read.cut().clone()))
    }
    .await;
    result.map_err(super::native::failure)
}
pub(super) async fn set(
    context: &ExecutionContext, id: &'static str, mutation: Option<wire::MutationId>, expected_revision: Vec<u8>, create_if_absent: bool,
    schema: Option<wire::SchemaInput>, value: Option<wire::ObjectValue>,
) -> Result<SetOutcome, ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let value: harmos_storage::ObjectValue = required(value)?.try_into()?;
        let source = ColumnSource {
            kind: ColumnKind::Objects,
            schema: value.schema.clone(),
        };
        super::native::admit(context, id, &source)?;
        let schema = required(schema)?;
        let key: harmos_storage::SchemaKey = required(schema.key)?.try_into()?;
        if key != value.schema {
            return Err(crate::Error::InvalidInput {
                field: "SetObject",
                reason: "schema differs from the object value",
            });
        }
        let graph = required(schema.graph)?.try_into()?;
        let expected = if create_if_absent && expected_revision.is_empty() {
            None
        } else if !create_if_absent && !expected_revision.is_empty() {
            let encoded = wire::ObjectVersionToken::decode(expected_revision.as_slice()).map_err(|_| crate::Error::InvalidInput {
                field: "SetObject",
                reason: "invalid expected revision; get again and retry",
            })?;
            let expected: harmos_storage::ObjectVersionToken = encoded.try_into()?;
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
        let identity = mutation.clone().try_into()?;
        let outcome = storage.compare_exchange_value_once(&identity, expected, value.clone(), graph).await?;
        Ok(match outcome {
            harmos_storage::CompareExchange::Applied(applied) => SetOutcome::Applied(
                value.into(),
                revision(applied.version),
                Box::new(super::transfers::receipt(&mutation, Some(applied.position))),
            ),
            harmos_storage::CompareExchange::Conflict { current } => SetOutcome::Conflict(current.map(revision).unwrap_or_default()),
        })
    }
    .await;
    result.map_err(super::native::failure)
}
