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

use super::requests::{IndexDefinition, MutationId, ProjectionDefinition, ReadLimits};

use super::requests::required;

use crate::execution::operations::{ExecutionContext, ServingError};
use harmos_protocol::v1;
use harmos_storage::{
    StorageManager,
    query::columnar::{Cancellation, ColumnSource},
};

/* === Structs === */

pub(super) struct SetIndexInput {
    pub definition: Option<IndexDefinition>,
    pub expected_revision: Vec<u8>,
    pub create_if_absent: bool,
    pub rebuild: bool,
    pub work: Option<ReadLimits>,
    pub mutation: Option<MutationId>,
}

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
    context: &ExecutionContext, name: String, source: Option<ColumnSource>, at: Option<v1::ReadCut>,
) -> Result<(IndexDefinition, Vec<u8>, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::admission::run(
        context,
        async {
            let source: ColumnSource = required(source)?;

            let read = snapshots.resolve(&storage, at).await?;
            let name = harmos_storage::Identifier::new(name).map_err(harmos_domain::Error::from)?;
            let (definition, current) = storage.get_index_definition(&name).await?;
            let resource = format!("storage.index.{name}");
            if super::lists::revision_at(read.cut(), &resource)?.is_some_and(|expected| expected != current) {
                return Err(harmos_storage::Error::NotReady { domain: "GetIndexDefinition revision" }.into());
            }
            super::admission::source_matches(&source, &definition.source)?;
            let cut = super::lists::with_revision(read.cut().clone(), &resource, current);
            drop(read);
            Ok((definition, revision(current), snapshots.composite(&storage, cut).await?))
        },
        cancel.clone(),
    )
    .await
}

pub(super) async fn set_index(
    context: &ExecutionContext, input: SetIndexInput,
) -> Result<(IndexDefinition, Vec<u8>, v1::Receipt), ServingError> {
    let SetIndexInput {
        definition,
        expected_revision,
        create_if_absent,
        rebuild,
        work,
        mutation,
    } = input;
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let expected = expected(&expected_revision, create_if_absent)?;
        let definition: harmos_storage::query::index::IndexDefinition = required(definition)?;
        definition.validate()?;

        let mutation = required(mutation)?;
        let identity: harmos_storage::MutationId = mutation.clone();
        let work: harmos_storage::ReadLimits = required(work)?;
        let input = harmos_storage::DefinitionInput::Index {
            value: definition.clone(),
            expected,
            rebuild,
            work,
        };
        if !storage.definition_admitted(&identity, &input)? {
            match storage.get_index_definition(&definition.name).await {
                Ok((_current, revision)) => {
                    if expected != Some(revision) {
                        return Err(conflict("Index"));
                    }
                }
                Err(harmos_storage::Error::IndexNotFound { .. }) if expected.is_none() => {}
                Err(harmos_storage::Error::NotReady { domain: "projection definition" }) if expected.is_none() => {}
                Err(error) => return Err(error.into()),
            }
        }
        let current = storage.set_index_definition_once(&identity, definition.clone(), expected, rebuild, work).await?;
        Ok((definition, revision(current), super::transfers::receipt(&mutation, None)))
    }
    .await;
    result.map_err(super::admission::failure)
}

pub(super) async fn get_projection(
    context: &ExecutionContext, name: String, source: Option<ColumnSource>, at: Option<v1::ReadCut>,
) -> Result<(ProjectionDefinition, Vec<u8>, v1::ReadCut), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
    let cancel = Cancellation::default();
    super::admission::run(
        context,
        async {
            let source: ColumnSource = required(source)?;

            let read = snapshots.resolve(&storage, at).await?;
            let name = harmos_storage::Identifier::new(name).map_err(harmos_domain::Error::from)?;
            let resource = format!("storage.projection.{name}");
            let expected = super::lists::revision_at(read.cut(), &resource)?;
            let (definition, current) = storage.get_projection_definition(&name, expected).await?;
            super::admission::source_matches(&source, &definition.source)?;
            let cut = super::lists::with_revision(read.cut().clone(), &resource, current);
            drop(read);
            Ok((definition, revision(current), snapshots.composite(&storage, cut).await?))
        },
        cancel.clone(),
    )
    .await
}

pub(super) async fn set_projection(
    context: &ExecutionContext, definition: Option<ProjectionDefinition>, expected_revision: Vec<u8>, create_if_absent: bool,
    rebuild: bool, mutation: Option<MutationId>,
) -> Result<(ProjectionDefinition, Vec<u8>, v1::Receipt), ServingError> {
    let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
    let result: crate::Result<_> = async {
        let expected = expected(&expected_revision, create_if_absent)?;
        let definition: harmos_storage::query::projection::ProjectionDefinition = required(definition)?;
        definition.validate()?;

        let mutation = required(mutation)?;
        let identity: harmos_storage::MutationId = mutation.clone();

        let input = harmos_storage::DefinitionInput::Projection {
            value: definition.clone(),
            expected,
            rebuild,
        };
        if !storage.definition_admitted(&identity, &input)? {
            match storage.get_projection_definition(&definition.name, None).await {
                Ok((_current, revision)) => {
                    if expected != Some(revision) {
                        return Err(conflict("Projection"));
                    }
                }
                Err(harmos_storage::Error::IndexNotFound { .. }) if expected.is_none() => {}
                Err(harmos_storage::Error::NotReady { domain: "projection definition" }) if expected.is_none() => {}
                Err(error) => return Err(error.into()),
            }
        }
        let current = storage.set_projection_definition_once(&identity, definition.clone(), expected, rebuild).await?;
        Ok((definition, revision(current), super::transfers::receipt(&mutation, None)))
    }
    .await;
    result.map_err(super::admission::failure)
}

fn conflict(kind: &'static str) -> crate::Error {
    harmos_storage::Error::backend(
        if kind == "Index" { "SetIndexDefinition" } else { "SetProjectionDefinition" },
        harmos_storage::BackendFailure::Conflict,
        std::io::Error::other("get again and retry"),
    )
    .into()
}
