/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Revision-Guarded Definition Handlers

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_protocol::{storage as wire, v1};
use harmos_storage::{
    StorageManager,
    query::{
        columnar::{Cancellation, ColumnSource},
        protobuf::required,
    },
};

/* === Public Functions === */

pub(super) fn expected(bytes: &[u8], create_if_absent: bool) -> crate::Result<Option<u64>> {
    if create_if_absent && bytes.is_empty() {
        return Ok(None);
    }
    if !create_if_absent && bytes.len() == 8 {
        let revision = u64::from_be_bytes(bytes.try_into().unwrap());
        if revision > 0 {
            return Ok(Some(revision));
        }
    }
    Err(crate::Error::InvalidInput {
        field: "Set definition",
        reason: "supply the Get revision or explicit create-if-absent",
    })
}

pub(super) fn revision(value: u64) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

pub(super) async fn get_index(
    context: &ExecutionContext, id: &'static str, name: String, source: Option<wire::ColumnSource>,
) -> Result<(wire::IndexDefinition, Vec<u8>, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::native::run(
        context,
        async {
            let source: ColumnSource = required(source)?.try_into()?;
            super::native::admit(context, id, &source)?;
            let read = snapshots.resolve(&storage, None).await?;
            let name = harmos_storage::Identifier::new(name).map_err(harmos_domain::Error::from)?;
            let (definition, current) = storage.get_index_definition(&name).await?;
            super::native::source_matches(&source, &definition.source)?;
            let mut cut = read.cut().clone();
            cut.revisions.push(v1::ResourceRevision {
                name: format!("storage.index.{}", definition.name),
                revision: revision(current),
            });
            Ok((definition.into(), revision(current), snapshots.composite(&storage, cut).await?))
        },
        cancel.clone(),
    )
    .await
}

pub(super) async fn set_index(
    context: &ExecutionContext, id: &'static str, definition: Option<wire::IndexDefinition>, expected_revision: Vec<u8>,
    create_if_absent: bool, rebuild: bool, work: Option<wire::ReadLimits>, mutation: Option<wire::MutationId>,
) -> Result<(wire::IndexDefinition, Vec<u8>, v1::Receipt), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let expected = expected(&expected_revision, create_if_absent)?;
        let definition: harmos_storage::query::index::IndexDefinition = required(definition)?.try_into()?;
        definition.validate()?;
        super::native::admit(context, id, &definition.source)?;
        let mutation = required(mutation)?;
        let identity: harmos_storage::MutationId = mutation.clone().try_into()?;
        let work: harmos_storage::ReadLimits = required(work)?.try_into()?;
        let input = harmos_storage::DefinitionInput::Index {
            value: definition.clone(),
            expected,
            rebuild,
            work,
        };
        if !storage.definition_admitted(&identity, &input)? {
            match storage.get_index_definition(&definition.name).await {
                Ok((current, revision)) => {
                    if expected != Some(revision) {
                        return Err(conflict("Index"));
                    }
                    super::native::admit(context, id, &current.source)?;
                }
                Err(harmos_storage::Error::IndexNotFound { .. }) if expected.is_none() => {}
                Err(harmos_storage::Error::NotReady { domain: "projection definition" }) if expected.is_none() => {}
                Err(error) => return Err(error.into()),
            }
        }
        let current = storage.set_index_definition_once(&identity, definition.clone(), expected, rebuild, work).await?;
        Ok((definition.into(), revision(current), super::transfers::receipt(&mutation, None)))
    }
    .await;
    result.map_err(super::native::failure)
}

pub(super) async fn get_projection(
    context: &ExecutionContext, id: &'static str, name: String, source: Option<wire::ColumnSource>,
) -> Result<(wire::ProjectionDefinition, Vec<u8>, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::native::run(
        context,
        async {
            let source: ColumnSource = required(source)?.try_into()?;
            super::native::admit(context, id, &source)?;
            let read = snapshots.resolve(&storage, None).await?;
            let name = harmos_storage::Identifier::new(name).map_err(harmos_domain::Error::from)?;
            let (definition, current) = storage.get_projection_definition(&name, None).await?;
            super::native::source_matches(&source, &definition.source)?;
            let mut cut = read.cut().clone();
            cut.revisions.push(v1::ResourceRevision {
                name: format!("storage.projection.{}", definition.name),
                revision: revision(current),
            });
            Ok((definition.try_into()?, revision(current), snapshots.composite(&storage, cut).await?))
        },
        cancel.clone(),
    )
    .await
}

pub(super) async fn set_projection(
    context: &ExecutionContext, id: &'static str, definition: Option<wire::ProjectionDefinition>, expected_revision: Vec<u8>,
    create_if_absent: bool, rebuild: bool, mutation: Option<wire::MutationId>,
) -> Result<(wire::ProjectionDefinition, Vec<u8>, v1::Receipt), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let expected = expected(&expected_revision, create_if_absent)?;
        let definition: harmos_storage::query::projection::ProjectionDefinition = required(definition)?.try_into()?;
        definition.validate()?;
        super::native::admit(context, id, &definition.source)?;
        let mutation = required(mutation)?;
        let identity: harmos_storage::MutationId = mutation.clone().try_into()?;

        let input = harmos_storage::DefinitionInput::Projection {
            value: definition.clone(),
            expected,
            rebuild,
        };
        if !storage.definition_admitted(&identity, &input)? {
            match storage.get_projection_definition(&definition.name, None).await {
                Ok((current, revision)) => {
                    if expected != Some(revision) {
                        return Err(conflict("Projection"));
                    }
                    super::native::admit(context, id, &current.source)?;
                }
                Err(harmos_storage::Error::IndexNotFound { .. }) if expected.is_none() => {}
                Err(harmos_storage::Error::NotReady { domain: "projection definition" }) if expected.is_none() => {}
                Err(error) => return Err(error.into()),
            }
        }
        let current = storage.set_projection_definition_once(&identity, definition.clone(), expected, rebuild).await?;
        Ok((definition.try_into()?, revision(current), super::transfers::receipt(&mutation, None)))
    }
    .await;
    result.map_err(super::native::failure)
}

fn conflict(kind: &'static str) -> crate::Error {
    harmos_storage::Error::backend(
        if kind == "Index" { "SetIndexDefinition" } else { "SetProjectionDefinition" },
        harmos_storage::BackendFailure::Conflict,
        std::io::Error::other("get again and retry"),
    )
    .into()
}
