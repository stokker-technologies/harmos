/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Private Native Storage Operations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::requests::{
    AdvanceProjectionRequest, AdvanceProjectionResponse, AppendFileBytesRequest, AppendFileBytesResponse, AppendObjectRevisionsRequest,
    AppendObjectRevisionsResponse, AppendRecordsRequest, AppendRecordsResponse, AppendSamplesRequest, AppendSamplesResponse,
    CapabilitiesRequest, CapabilitiesResponse, CaptureFileRequest, CaptureFileResponse, CreateFileProductionRequest,
    CreateFileProductionResponse, CreateFileWriterRequest, CreateFileWriterResponse, CreateSnapshotRequest, CreateSnapshotResponse,
    DeleteFileProductionRequest, DeleteFileProductionResponse, DeleteSnapshotRequest, DeleteSnapshotResponse, GetFileBytesRequest,
    GetFileBytesResponse, GetFileHashRequest, GetFileHashResponse, GetFileProductionRequest, GetFileProductionResponse,
    GetIndexDefinitionRequest, GetIndexDefinitionResponse, GetObjectRequest, GetObjectResponse, GetProjectionDefinitionRequest,
    GetProjectionDefinitionResponse, GetProjectionStatusRequest, GetProjectionStatusResponse, ListColumnRowsRequest,
    ListColumnRowsResponse, ListDirectoryEntriesRequest, ListDirectoryEntriesResponse, ListIndexEntriesRequest, ListIndexEntriesResponse,
    ListProjectionRowsRequest, ListProjectionRowsResponse, ListRowsRequest, ListRowsResponse, PublishFileProductionRequest,
    PublishFileProductionResponse, SetIndexDefinitionRequest, SetIndexDefinitionResponse, SetObjectRequest, SetObjectResponse,
    SetProjectionDefinitionRequest, SetProjectionDefinitionResponse, WatchDirectoryEntriesRequest, WatchDirectoryEntriesResponse,
    WatchFileBytesRequest, WatchFileBytesResponse, WatchFileProductionRequest, WatchFileProductionResponse, WatchRowsRequest,
    WatchRowsResponse, set_object_response, watch_file_production_response, watch_rows_response,
};

// Standard Library Imports

// Third Party Imports

// Local Imports
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
    type Request = CreateFileProductionRequest;
    type Response = CreateFileProductionResponse;
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
    type Request = GetFileProductionRequest;
    type Response = GetFileProductionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_file_production";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (production, at) = super::file_access::production(&context, request.file, request.at).await?;
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
    type Request = AppendFileBytesRequest;
    type Response = AppendFileBytesResponse;
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
    type Request = CreateFileWriterRequest;
    type Response = CreateFileWriterResponse;
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
    type Request = PublishFileProductionRequest;
    type Response = PublishFileProductionResponse;
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
    type Request = DeleteFileProductionRequest;
    type Response = DeleteFileProductionResponse;
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
    type Request = GetFileBytesRequest;
    type Response = GetFileBytesResponse;
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
    type Request = GetIndexDefinitionRequest;
    type Response = GetIndexDefinitionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_index_definition";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, resolved_at) = super::definitions::get_index(&context, request.name, request.source, request.at).await?;
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
    type Request = SetIndexDefinitionRequest;
    type Response = SetIndexDefinitionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.set_index_definition";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, receipt) = super::definitions::set_index(
            &context,
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
    type Request = GetProjectionDefinitionRequest;
    type Response = GetProjectionDefinitionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_projection_definition";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, resolved_at) =
            super::definitions::get_projection(&context, request.name, request.source, request.at).await?;
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
    type Request = SetProjectionDefinitionRequest;
    type Response = SetProjectionDefinitionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.set_projection_definition";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, receipt) = super::definitions::set_projection(
            &context,
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
    type Request = ListRowsRequest;
    type Response = ListRowsResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.list_rows";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let page = super::pages::list(&context, request.source, request.at, request.transforms, request.page).await?;
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
    type Request = CapabilitiesRequest;
    type Response = CapabilitiesResponse;
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
    type Request = CreateSnapshotRequest;
    type Response = CreateSnapshotResponse;
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
    type Request = DeleteSnapshotRequest;
    type Response = DeleteSnapshotResponse;
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
    type Request = ListColumnRowsRequest;
    type Response = ListColumnRowsResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.list_column_rows";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (page, resolved_at, next_page_token) = super::lists::columns(&context, request.selection, request.at, request.page).await?;
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
    type Request = ListIndexEntriesRequest;
    type Response = ListIndexEntriesResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.list_index_entries";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (page, resolved_at, next_page_token) =
            super::lists::indexes(&context, request.selection, request.source, request.at, request.page).await?;
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
    type Request = AdvanceProjectionRequest;
    type Response = AdvanceProjectionResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.advance_projection";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (revision, resolved_at) =
            super::views::advance_projection(&context, request.snapshot, request.name, request.work, request.source).await?;
        super::admission::read_response(&context, Self::Response { revision, resolved_at: Some(resolved_at) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct GetProjectionStatus;
impl NativeOperation for GetProjectionStatus {
    type Request = GetProjectionStatusRequest;
    type Response = GetProjectionStatusResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_projection_status";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (status, resolved_at) =
            super::views::projection_status(&context, request.name, request.revision, request.at, request.source).await?;
        super::admission::read_response(&context, Self::Response { status, resolved_at: Some(resolved_at) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct ListProjectionRows;
impl NativeOperation for ListProjectionRows {
    type Request = ListProjectionRowsRequest;
    type Response = ListProjectionRowsResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.list_projection_rows";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (page, resolved_at, next_page_token) =
            super::lists::projections(&context, request.selection, request.source, request.at, request.page).await?;
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
    type Request = GetObjectRequest;
    type Response = GetObjectResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_object";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (value, revision, resolved_at) = super::object::get(&context, request.schema, request.item, request.at).await?;
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
    type Request = SetObjectRequest;
    type Response = SetObjectResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.set_object";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let result = super::object::set(
            &context,
            request.mutation,
            request.expected_revision,
            request.create_if_absent,
            request.schema,
            request.value,
        )
        .await?;
        use set_object_response::{Applied, Conflict, Outcome};
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
    type Request = AppendRecordsRequest;
    type Response = AppendRecordsResponse;
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
    type Request = AppendSamplesRequest;
    type Response = AppendSamplesResponse;
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
    type Request = AppendObjectRevisionsRequest;
    type Response = AppendObjectRevisionsResponse;
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
    type Request = WatchRowsRequest;
    type Response = WatchRowsResponse;
    type Endpoints = super::NativeOutput<WatchRowsResponse>;
    const ID: &'static str = "storage.watch_rows";
    const POLICY: crate::OperationPolicy = super::STREAM_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let context = &context;
        let frame = super::watch_rows::run(context, request.source, request.at, |frame| async move {
            let response = super::admission::read_response(context, Self::response(frame))?;
            output.send(response).await.map_err(ServingError::Stream)
        })
        .await?;
        Ok(Self::response(frame))
    }
}
impl WatchRows {
    fn response(frame: super::watch_rows::Frame) -> WatchRowsResponse {
        use super::watch_rows::Event;
        use watch_rows_response::Event as Wire;
        WatchRowsResponse {
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
    type Request = WatchFileBytesRequest;
    type Response = WatchFileBytesResponse;
    type Endpoints = super::NativeOutput<WatchFileBytesResponse>;
    const ID: &'static str = "storage.watch_file_bytes";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let context = &context;
        let completion = super::file_watches::bytes(context, request.source, request.start, |event| async move {
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
    type Request = WatchFileProductionRequest;
    type Response = WatchFileProductionResponse;
    type Endpoints = super::NativeOutput<WatchFileProductionResponse>;
    const ID: &'static str = "storage.watch_file_production";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let context = &context;
        let progress = super::file_watches::production(context, request.file, request.offset, |data| async move {
            super::file_watches::send(
                context,
                Self::ID,
                output,
                Self::Response {
                    event: Some(watch_file_production_response::Event::Data(data)),
                },
            )
            .await
        })
        .await?;
        let response = Self::Response {
            event: Some(watch_file_production_response::Event::Progress(progress)),
        };
        super::file_watches::send(context, Self::ID, output, response.clone()).await?;
        Ok(response)
    }
}

pub struct WatchDirectoryEntries;
impl NativeOperation for WatchDirectoryEntries {
    type Request = WatchDirectoryEntriesRequest;
    type Response = WatchDirectoryEntriesResponse;
    type Endpoints = super::NativeOutput<WatchDirectoryEntriesResponse>;
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
    type Request = GetFileHashRequest;
    type Response = GetFileHashResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_file_hash";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (hash, at) = super::file_access::hash(&context, request.source, request.range, request.at).await?;
        super::admission::read_response(&context, Self::Response { hash: Some(hash), resolved_at: Some(at) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct CaptureFile;
impl NativeOperation for CaptureFile {
    type Request = CaptureFileRequest;
    type Response = CaptureFileResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.capture_file";
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (file, receipt) =
            super::file_access::capture(&context, request.source, request.range, request.metadata, request.consistency).await?;
        super::admission::read_response(&context, Self::Response { file: Some(file), receipt: Some(receipt) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct ListDirectoryEntries;
impl NativeOperation for ListDirectoryEntries {
    type Request = ListDirectoryEntriesRequest;
    type Response = ListDirectoryEntriesResponse;
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
