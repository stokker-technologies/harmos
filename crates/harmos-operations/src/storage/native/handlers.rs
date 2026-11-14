/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Client Storage Transfer Declarations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::requests as wire;
use super::transfers;
use crate::execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError};
use crate::operations::{FailureKind, ReplaySafety};
use harmos_storage::StorageManager;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct CreateFileProduction;

pub struct GetFileProduction;

pub struct AppendFileBytes;

pub struct CreateFileWriter;

pub struct PublishFileProduction;

pub struct DeleteFileProduction;

pub struct GetFileBytes;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl NativeOperation for CreateFileProduction {
    type Request = wire::CreateFileProductionRequest;
    type Response = wire::CreateFileProductionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.create_file_production";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            let (production, receipt) =
                transfers::create(&storage, transfers::required(request.metadata)?, transfers::required(request.mutation)?).await?;
            Ok(Self::Response {
                production: Some(production),
                receipt: Some(receipt),
            })
        }
        .await;
        result.map_err(super::admission::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl NativeOperation for GetFileProduction {
    type Request = wire::GetFileProductionRequest;
    type Response = wire::GetFileProductionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_file_production";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (production, at) = super::file_access::production(&context, Self::ID, request.file, request.at).await?;
        super::admission::read_response(
            &context,
            Self::Response {
                production: Some(production.progress),
                writer_generation: production.generation,
                resolved_at: Some(at),
                accepted_length: production.accepted,
                durable_length: production.durable,
            },
        )
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl NativeOperation for AppendFileBytes {
    type Request = wire::AppendFileBytesRequest;
    type Response = wire::AppendFileBytesResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.append_file_bytes";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            let (production, receipt) = transfers::append(
                &storage,
                transfers::required(request.session)?,
                transfers::required(request.mutation)?,
                request.offset,
                request.data,
            )
            .await?;
            Ok(Self::Response {
                production: Some(production),
                receipt: Some(receipt),
            })
        }
        .await;
        result.map_err(super::admission::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl NativeOperation for CreateFileWriter {
    type Request = wire::CreateFileWriterRequest;
    type Response = wire::CreateFileWriterResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.create_file_writer";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            let (production, receipt) =
                transfers::writer(&storage, transfers::required(request.session)?, transfers::required(request.mutation)?).await?;
            Ok(Self::Response {
                production: Some(production),
                receipt: Some(receipt),
            })
        }
        .await;
        result.map_err(super::admission::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl NativeOperation for PublishFileProduction {
    type Request = wire::PublishFileProductionRequest;
    type Response = wire::PublishFileProductionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.publish_file_production";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            let (file, receipt) = transfers::publish(
                &storage,
                transfers::required(request.session)?,
                transfers::required(request.mutation)?,
                request.length,
                request.sha256,
            )
            .await?;
            Ok(Self::Response { file: Some(file), receipt: Some(receipt) })
        }
        .await;
        result.map_err(super::admission::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl NativeOperation for DeleteFileProduction {
    type Request = wire::DeleteFileProductionRequest;
    type Response = wire::DeleteFileProductionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.delete_file_production";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            let (production, receipt) =
                transfers::delete(&storage, transfers::required(request.session)?, transfers::required(request.mutation)?).await?;
            Ok(Self::Response {
                production: Some(production),
                receipt: Some(receipt),
            })
        }
        .await;
        result.map_err(super::admission::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl NativeOperation for GetFileBytes {
    type Request = wire::GetFileBytesRequest;
    type Response = wire::GetFileBytesResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_file_bytes";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            let scope = context.call.get::<crate::operations::Scope>();
            let (range, resolved_at) = transfers::get(
                &storage,
                &snapshots,
                transfers::required(request.file)?,
                request.offset,
                request.max_bytes,
                request.at,
                scope.map(crate::operations::Scope::as_str),
            )
            .await?;
            let mut response = Self::Response {
                data: range.data,
                total_length: range.total_length,
                sha256: range.sha256.to_vec(),
                resolved_at: Some(resolved_at),
            };
            let length = transfers::range_bytes(
                crate::MemorySize::memory_size(&response),
                response.data.len(),
                storage.settings().transfer_message_bytes().get(),
            )?;
            response.data.truncate(length);
            Ok(response)
        }
        .await;
        result.map_err(super::admission::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

pub struct GetIndexDefinition;

impl NativeOperation for GetIndexDefinition {
    type Request = wire::GetIndexDefinitionRequest;
    type Response = wire::GetIndexDefinitionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_index_definition";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, resolved_at) =
            super::definitions::get_index(&context, Self::ID, request.name, request.source, request.at).await?;
        super::admission::read_response(
            &context,
            Self::Response {
                definition: Some(definition),
                revision,
                resolved_at: Some(resolved_at),
            },
        )
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct SetIndexDefinition;

impl NativeOperation for SetIndexDefinition {
    type Request = wire::SetIndexDefinitionRequest;
    type Response = wire::SetIndexDefinitionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.set_index_definition";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, receipt) = super::definitions::set_index(
            &context,
            Self::ID,
            super::definitions::SetIndexInput {
                definition: request.definition,
                expected_revision: request.expected_revision,
                create_if_absent: request.create_if_absent,
                rebuild: request.rebuild,
                work: request.work,
                mutation: request.mutation,
            },
        )
        .await?;
        super::admission::read_response(
            &context,
            Self::Response {
                definition: Some(definition),
                revision,
                receipt: Some(receipt),
            },
        )
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct GetProjectionDefinition;

impl NativeOperation for GetProjectionDefinition {
    type Request = wire::GetProjectionDefinitionRequest;
    type Response = wire::GetProjectionDefinitionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_projection_definition";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, resolved_at) =
            super::definitions::get_projection(&context, Self::ID, request.name, request.source, request.at).await?;
        super::admission::read_response(
            &context,
            Self::Response {
                definition: Some(definition),
                revision,
                resolved_at: Some(resolved_at),
            },
        )
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct SetProjectionDefinition;

impl NativeOperation for SetProjectionDefinition {
    type Request = wire::SetProjectionDefinitionRequest;
    type Response = wire::SetProjectionDefinitionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.set_projection_definition";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, receipt) = super::definitions::set_projection(
            &context,
            Self::ID,
            request.definition,
            request.expected_revision,
            request.create_if_absent,
            request.rebuild,
            request.mutation,
        )
        .await?;
        super::admission::read_response(
            &context,
            Self::Response {
                definition: Some(definition),
                revision,
                receipt: Some(receipt),
            },
        )
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct ListRows;

impl NativeOperation for ListRows {
    type Request = wire::ListRowsRequest;
    type Response = wire::ListRowsResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.list_rows";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let page = super::pages::list(&context, Self::ID, request.source, request.at, request.transforms, request.page).await?;
        super::admission::read_response(
            &context,
            Self::Response {
                rows: page.rows,
                resolved_at: Some(page.resolved_at),
                next_page_token: page.next_page_token,
            },
        )
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct Capabilities;
impl NativeOperation for Capabilities {
    type Request = wire::CapabilitiesRequest;
    type Response = wire::CapabilitiesResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.capabilities";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, _: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (capabilities, mutations, file_batch_bytes) = super::views::capabilities(&context).await?;
        super::admission::read_response(
            &context,
            Self::Response {
                capabilities: Some(capabilities),
                mutations,
                file_batch_bytes,
            },
        )
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct CreateSnapshot;
impl NativeOperation for CreateSnapshot {
    type Request = wire::CreateSnapshotRequest;
    type Response = wire::CreateSnapshotResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.create_snapshot";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (snapshot, resolved_at) = super::views::create(&context, request.at).await?;
        super::admission::read_response(
            &context,
            Self::Response {
                snapshot: Some(snapshot),
                resolved_at: Some(resolved_at),
            },
        )
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct DeleteSnapshot;
impl NativeOperation for DeleteSnapshot {
    type Request = wire::DeleteSnapshotRequest;
    type Response = wire::DeleteSnapshotResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.delete_snapshot";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        super::views::delete(&context, request.snapshot).await?;
        super::admission::read_response(&context, Self::Response {})
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct ListColumnRows;
impl NativeOperation for ListColumnRows {
    type Request = wire::ListColumnRowsRequest;
    type Response = wire::ListColumnRowsResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.list_column_rows";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (page, resolved_at, next_page_token) =
            super::lists::columns(&context, Self::ID, request.selection, request.at, request.page).await?;
        super::admission::read_response(
            &context,
            Self::Response {
                page: Some(page),
                resolved_at: Some(resolved_at),
                next_page_token,
            },
        )
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct ListIndexEntries;
impl NativeOperation for ListIndexEntries {
    type Request = wire::ListIndexEntriesRequest;
    type Response = wire::ListIndexEntriesResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.list_index_entries";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (page, resolved_at, next_page_token) =
            super::lists::indexes(&context, Self::ID, request.selection, request.source, request.at, request.page).await?;
        super::admission::read_response(
            &context,
            Self::Response {
                page: Some(page),
                resolved_at: Some(resolved_at),
                next_page_token,
            },
        )
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct AdvanceProjection;
impl NativeOperation for AdvanceProjection {
    type Request = wire::AdvanceProjectionRequest;
    type Response = wire::AdvanceProjectionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.advance_projection";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (revision, resolved_at) =
            super::views::advance_projection(&context, Self::ID, request.snapshot, request.name, request.work, request.source).await?;
        super::admission::read_response(&context, Self::Response { revision, resolved_at: Some(resolved_at) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct GetProjectionStatus;
impl NativeOperation for GetProjectionStatus {
    type Request = wire::GetProjectionStatusRequest;
    type Response = wire::GetProjectionStatusResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_projection_status";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (status, resolved_at) =
            super::views::projection_status(&context, Self::ID, request.name, request.revision, request.at, request.source).await?;
        super::admission::read_response(&context, Self::Response { status, resolved_at: Some(resolved_at) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct ListProjectionRows;
impl NativeOperation for ListProjectionRows {
    type Request = wire::ListProjectionRowsRequest;
    type Response = wire::ListProjectionRowsResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.list_projection_rows";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (page, resolved_at, next_page_token) =
            super::lists::projections(&context, Self::ID, request.selection, request.source, request.at, request.page).await?;
        super::admission::read_response(
            &context,
            Self::Response {
                page: Some(page),
                resolved_at: Some(resolved_at),
                next_page_token,
            },
        )
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

/* === Object Mirrors === */

pub struct GetObject;
pub struct SetObject;

impl NativeOperation for GetObject {
    type Request = wire::GetObjectRequest;
    type Response = wire::GetObjectResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_object";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (value, revision, resolved_at) = super::object::get(&context, Self::ID, request.schema, request.item, request.at).await?;
        super::admission::read_response(
            &context,
            Self::Response {
                exists: value.is_some(),
                value,
                revision,
                resolved_at: Some(resolved_at),
            },
        )
    }
}
impl NativeOperation for SetObject {
    type Request = wire::SetObjectRequest;
    type Response = wire::SetObjectResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.set_object";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let result = super::object::set(
            &context,
            Self::ID,
            request.mutation,
            request.expected_revision,
            request.create_if_absent,
            request.schema,
            request.value,
        )
        .await?;
        use wire::set_object_response::{Applied, Conflict, Outcome};
        let outcome = match result {
            super::object::SetOutcome::Applied(value, revision, receipt) => Outcome::Applied(Applied {
                value: Some(*value),
                revision,
                receipt: Some(receipt),
            }),
            super::object::SetOutcome::Conflict(current_revision) => Outcome::Conflict(Conflict {
                current_revision,
                action: "SetObject: get again and retry".into(),
            }),
        };
        super::admission::read_response(&context, Self::Response { outcome: Some(outcome) })
    }
}

/* === Immutable Append === */

pub struct AppendRecords;
impl NativeOperation for AppendRecords {
    type Request = wire::AppendRecordsRequest;
    type Response = wire::AppendRecordsResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.append_records";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let receipt =
            super::append::append(&context, Self::ID, super::append::Kind::Records, request.mutation, request.scope, request.batch).await?;
        super::admission::read_response(&context, Self::Response { receipt: Some(receipt) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
pub struct AppendSamples;
impl NativeOperation for AppendSamples {
    type Request = wire::AppendSamplesRequest;
    type Response = wire::AppendSamplesResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.append_samples";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let receipt =
            super::append::append(&context, Self::ID, super::append::Kind::Samples, request.mutation, request.scope, request.batch).await?;
        super::admission::read_response(&context, Self::Response { receipt: Some(receipt) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
pub struct AppendObjectRevisions;
impl NativeOperation for AppendObjectRevisions {
    type Request = wire::AppendObjectRevisionsRequest;
    type Response = wire::AppendObjectRevisionsResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.append_object_revisions";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let receipt =
            super::append::append(&context, Self::ID, super::append::Kind::Objects, request.mutation, request.scope, request.batch).await?;
        super::admission::read_response(&context, Self::Response { receipt: Some(receipt) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct WatchRows;

impl NativeOperation for WatchRows {
    type Request = wire::WatchRowsRequest;
    type Response = wire::WatchRowsResponse;
    type Endpoints = super::NativeOutput<wire::WatchRowsResponse>;
    const ID: &'static str = "storage.watch_rows";
    const POLICY: crate::OperationPolicy = super::STREAM_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let context = &context;
        let frame = super::watch_rows::run(context, Self::ID, request.source, request.at, |frame| async move {
            let response = super::admission::read_response(context, Self::response(frame))?;
            output.send(response).await.map_err(ServingError::Stream)
        })
        .await?;
        Ok(Self::response(frame))
    }
}
impl WatchRows {
    fn response(frame: super::watch_rows::Frame) -> wire::WatchRowsResponse {
        use super::watch_rows::Event;
        use wire::watch_rows_response::Event as Wire;
        wire::WatchRowsResponse {
            resolved_at: Some(frame.at),
            event: Some(match frame.event {
                Event::Boundary => Wire::Boundary(()),
                Event::Record(row) => Wire::Record(row),
                Event::Sample(row) => Wire::Sample(row),
                Event::Object(row) => Wire::Object(row),
                Event::Progress => Wire::DurableProgress(()),
                Event::Resync(error) => Wire::ResyncRequired(error),
            }),
        }
    }
}

pub struct WatchFileBytes;
impl NativeOperation for WatchFileBytes {
    type Request = wire::WatchFileBytesRequest;
    type Response = wire::WatchFileBytesResponse;
    type Endpoints = super::NativeOutput<wire::WatchFileBytesResponse>;
    const ID: &'static str = "storage.watch_file_bytes";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let context = &context;
        let completion = super::file_watches::bytes(context, Self::ID, request.source, request.start, |event| async move {
            super::file_watches::send(context, Self::ID, output, Self::Response { event: Some(event), completion: None }).await
        })
        .await?;
        let response = Self::Response {
            event: None,
            completion: Some(completion),
        };
        super::file_watches::send(context, Self::ID, output, response.clone()).await?;
        Ok(response)
    }
}

pub struct WatchFileProduction;
impl NativeOperation for WatchFileProduction {
    type Request = wire::WatchFileProductionRequest;
    type Response = wire::WatchFileProductionResponse;
    type Endpoints = super::NativeOutput<wire::WatchFileProductionResponse>;
    const ID: &'static str = "storage.watch_file_production";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let context = &context;
        let progress = super::file_watches::production(context, Self::ID, request.file, request.offset, |data| async move {
            super::file_watches::send(
                context,
                Self::ID,
                output,
                Self::Response {
                    event: Some(wire::watch_file_production_response::Event::Data(data)),
                },
            )
            .await
        })
        .await?;
        let response = Self::Response {
            event: Some(wire::watch_file_production_response::Event::Progress(progress)),
        };
        super::file_watches::send(context, Self::ID, output, response.clone()).await?;
        Ok(response)
    }
}

pub struct WatchDirectoryEntries;
impl NativeOperation for WatchDirectoryEntries {
    type Request = wire::WatchDirectoryEntriesRequest;
    type Response = wire::WatchDirectoryEntriesResponse;
    type Endpoints = super::NativeOutput<wire::WatchDirectoryEntriesResponse>;
    const ID: &'static str = "storage.watch_directory_entries";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let context = &context;
        super::file_watches::directory(context, request.selection, request.include_existing, |event| async move {
            super::file_watches::send(context, Self::ID, output, Self::Response { event: Some(event) }).await
        })
        .await?;
        Ok(Self::Response { event: None })
    }
}

pub struct GetFileHash;
impl NativeOperation for GetFileHash {
    type Request = wire::GetFileHashRequest;
    type Response = wire::GetFileHashResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_file_hash";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (hash, at) = super::file_access::hash(&context, Self::ID, request.source, request.range, request.at).await?;
        super::admission::read_response(&context, Self::Response { hash: Some(hash), resolved_at: Some(at) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct CaptureFile;
impl NativeOperation for CaptureFile {
    type Request = wire::CaptureFileRequest;
    type Response = wire::CaptureFileResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.capture_file";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (file, receipt) =
            super::file_access::capture(&context, Self::ID, request.source, request.range, request.metadata, request.consistency).await?;
        super::admission::read_response(&context, Self::Response { file: Some(file), receipt: Some(receipt) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct ListDirectoryEntries;
impl NativeOperation for ListDirectoryEntries {
    type Request = wire::ListDirectoryEntriesRequest;
    type Response = wire::ListDirectoryEntriesResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.list_directory_entries";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        super::file_access::directory(&context, request.selection, request.at, request.page).await
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
