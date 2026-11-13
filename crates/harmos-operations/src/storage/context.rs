/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Context Storage Transfer Declarations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::transfers;
use crate::execution::operations::{ExecutionContext, ProtobufOperation, Resource, ServingError, Visibility, access::GrantAccess};
use crate::operations::{FailureKind, ReplaySafety};
use harmos_protocol::context as wire;
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

impl ProtobufOperation for CreateFileProduction {
    type Request = wire::StorageCreateFileProductionRequest;
    type Response = wire::StorageCreateFileProductionResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/CreateFileProduction";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];

    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        transfers::grant(Self::ID, request.metadata.as_ref().map(|value| value.id.as_str()))
    }

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            Self::grant(&request).map_err(|_| crate::Error::InvalidInput {
                field: "CreateFileProduction",
                reason: "file identity is required",
            })?;
            transfers::admit(
                &context,
                Self::ID,
                request.metadata.as_ref().map(|value| value.id.as_str()).expect("validated file identity"),
            )?;
            let (production, receipt) =
                transfers::create(&storage, transfers::required(request.metadata)?, transfers::required(request.mutation)?).await?;
            Ok(Self::Response {
                production: Some(production),
                receipt: Some(receipt),
            })
        }
        .await;
        result.map_err(super::native::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl ProtobufOperation for GetFileProduction {
    type Request = wire::StorageGetFileProductionRequest;
    type Response = wire::StorageGetFileProductionResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/GetFileProduction";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        transfers::grant(Self::ID, request.file.as_ref().map(|value| value.id.as_str()))
    }

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (production, at) = super::file_access::production(&context, Self::ID, request.file, request.at).await?;
        super::native::read_response(
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

impl ProtobufOperation for AppendFileBytes {
    type Request = wire::StorageAppendFileBytesRequest;
    type Response = wire::StorageAppendFileBytesResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/AppendFileBytes";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];

    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        transfers::grant(Self::ID, request.session.as_ref().map(|value| value.id.as_str()))
    }

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            Self::grant(&request).map_err(|_| crate::Error::InvalidInput {
                field: "AppendFileBytes",
                reason: "file identity is required",
            })?;
            transfers::admit(
                &context,
                Self::ID,
                request.session.as_ref().map(|value| value.id.as_str()).expect("validated file identity"),
            )?;
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
        result.map_err(super::native::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl ProtobufOperation for CreateFileWriter {
    type Request = wire::StorageCreateFileWriterRequest;
    type Response = wire::StorageCreateFileWriterResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/CreateFileWriter";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];

    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        transfers::grant(Self::ID, request.session.as_ref().map(|value| value.id.as_str()))
    }

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            Self::grant(&request).map_err(|_| crate::Error::InvalidInput {
                field: "CreateFileWriter",
                reason: "file identity is required",
            })?;
            transfers::admit(
                &context,
                Self::ID,
                request.session.as_ref().map(|value| value.id.as_str()).expect("validated file identity"),
            )?;
            let (production, receipt) =
                transfers::writer(&storage, transfers::required(request.session)?, transfers::required(request.mutation)?).await?;
            Ok(Self::Response {
                production: Some(production),
                receipt: Some(receipt),
            })
        }
        .await;
        result.map_err(super::native::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl ProtobufOperation for PublishFileProduction {
    type Request = wire::StoragePublishFileProductionRequest;
    type Response = wire::StoragePublishFileProductionResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/PublishFileProduction";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];

    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        transfers::grant(Self::ID, request.session.as_ref().map(|value| value.id.as_str()))
    }

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            Self::grant(&request).map_err(|_| crate::Error::InvalidInput {
                field: "PublishFileProduction",
                reason: "file identity is required",
            })?;
            transfers::admit(
                &context,
                Self::ID,
                request.session.as_ref().map(|value| value.id.as_str()).expect("validated file identity"),
            )?;
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
        result.map_err(super::native::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl ProtobufOperation for DeleteFileProduction {
    type Request = wire::StorageDeleteFileProductionRequest;
    type Response = wire::StorageDeleteFileProductionResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/DeleteFileProduction";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];

    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        transfers::grant(Self::ID, request.session.as_ref().map(|value| value.id.as_str()))
    }

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            Self::grant(&request).map_err(|_| crate::Error::InvalidInput {
                field: "DeleteFileProduction",
                reason: "file identity is required",
            })?;
            transfers::admit(
                &context,
                Self::ID,
                request.session.as_ref().map(|value| value.id.as_str()).expect("validated file identity"),
            )?;
            let (production, receipt) =
                transfers::delete(&storage, transfers::required(request.session)?, transfers::required(request.mutation)?).await?;
            Ok(Self::Response {
                production: Some(production),
                receipt: Some(receipt),
            })
        }
        .await;
        result.map_err(super::native::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl ProtobufOperation for GetFileBytes {
    type Request = wire::StorageGetFileBytesRequest;
    type Response = wire::StorageGetFileBytesResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/GetFileBytes";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        transfers::grant(Self::ID, request.file.as_ref().and_then(|value| value.metadata.as_ref()).map(|value| value.id.as_str()))
    }

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let snapshots = context.resource::<super::snapshot::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            Self::grant(&request).map_err(|_| crate::Error::InvalidInput {
                field: "GetFileBytes",
                reason: "file identity is required",
            })?;
            transfers::admit(
                &context,
                Self::ID,
                request
                    .file
                    .as_ref()
                    .and_then(|value| value.metadata.as_ref())
                    .map(|value| value.id.as_str())
                    .expect("validated file identity"),
            )?;
            let (range, resolved_at) =
                transfers::get(&storage, &snapshots, transfers::required(request.file)?, request.offset, request.max_bytes, request.at)
                    .await?;
            let mut response = Self::Response {
                data: range.data,
                total_length: range.total_length,
                sha256: range.sha256.to_vec(),
                resolved_at: Some(resolved_at),
            };
            let length = transfers::range_bytes(
                prost::Message::encoded_len(&response),
                response.data.len(),
                storage.settings().response.batch_bytes.get(),
            )?;
            response.data.truncate(length);
            Ok(response)
        }
        .await;
        result.map_err(super::native::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

pub struct GetIndexDefinition;

impl ProtobufOperation for GetIndexDefinition {
    type Request = wire::StorageGetIndexDefinitionRequest;
    type Response = wire::StorageGetIndexDefinitionResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/GetIndexDefinition";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.source.clone())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, resolved_at) =
            super::definitions::get_index(&context, Self::ID, request.name, request.source, request.at).await?;
        super::native::read_response(
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

impl ProtobufOperation for SetIndexDefinition {
    type Request = wire::StorageSetIndexDefinitionRequest;
    type Response = wire::StorageSetIndexDefinitionResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/SetIndexDefinition";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.definition.as_ref().and_then(|definition| definition.source.clone()))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, receipt) = super::definitions::set_index(
            &context,
            Self::ID,
            request.definition,
            request.expected_revision,
            request.create_if_absent,
            request.rebuild,
            request.work,
            request.mutation,
        )
        .await?;
        super::native::read_response(
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

impl ProtobufOperation for GetProjectionDefinition {
    type Request = wire::StorageGetProjectionDefinitionRequest;
    type Response = wire::StorageGetProjectionDefinitionResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/GetProjectionDefinition";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.source.clone())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, resolved_at) =
            super::definitions::get_projection(&context, Self::ID, request.name, request.source, request.at).await?;
        super::native::read_response(
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

impl ProtobufOperation for SetProjectionDefinition {
    type Request = wire::StorageSetProjectionDefinitionRequest;
    type Response = wire::StorageSetProjectionDefinitionResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/SetProjectionDefinition";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.definition.as_ref().and_then(|definition| definition.source.clone()))
    }
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
        super::native::read_response(
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

impl ProtobufOperation for ListRows {
    type Request = wire::StorageListRowsRequest;
    type Response = wire::StorageListRowsResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/ListRows";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::pages::grant(Self::ID, request.source.clone())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let page = super::pages::list(&context, Self::ID, request.source, request.at, request.transforms, request.page).await?;
        super::native::read_response(
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
impl ProtobufOperation for Capabilities {
    type Request = wire::StorageCapabilitiesRequest;
    type Response = wire::StorageCapabilitiesResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/Capabilities";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, _: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (capabilities, mutations, file_batch_bytes) = super::views::capabilities(&context).await?;
        super::native::read_response(
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
impl ProtobufOperation for CreateSnapshot {
    type Request = wire::StorageCreateSnapshotRequest;
    type Response = wire::StorageCreateSnapshotResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/CreateSnapshot";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (snapshot, resolved_at) = super::views::create(&context, request.at).await?;
        super::native::read_response(
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
impl ProtobufOperation for DeleteSnapshot {
    type Request = wire::StorageDeleteSnapshotRequest;
    type Response = wire::StorageDeleteSnapshotResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/DeleteSnapshot";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        super::views::delete(&context, request.snapshot).await?;
        super::native::read_response(&context, Self::Response {})
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct ListColumnRows;
impl ProtobufOperation for ListColumnRows {
    type Request = wire::StorageListColumnRowsRequest;
    type Response = wire::StorageListColumnRowsResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/ListColumnRows";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.selection.as_ref().and_then(|read| read.source.clone()))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (page, resolved_at, next_page_token) =
            super::lists::columns(&context, Self::ID, request.selection, request.at, request.page).await?;
        super::native::read_response(
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
impl ProtobufOperation for ListIndexEntries {
    type Request = wire::StorageListIndexEntriesRequest;
    type Response = wire::StorageListIndexEntriesResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/ListIndexEntries";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.source.clone())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (page, resolved_at, next_page_token) =
            super::lists::indexes(&context, Self::ID, request.selection, request.source, request.at, request.page).await?;
        super::native::read_response(
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
impl ProtobufOperation for AdvanceProjection {
    type Request = wire::StorageAdvanceProjectionRequest;
    type Response = wire::StorageAdvanceProjectionResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/AdvanceProjection";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.source.clone())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (revision, resolved_at) =
            super::views::advance_projection(&context, Self::ID, request.snapshot, request.name, request.work, request.source).await?;
        super::native::read_response(&context, Self::Response { revision, resolved_at: Some(resolved_at) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct GetProjectionStatus;
impl ProtobufOperation for GetProjectionStatus {
    type Request = wire::StorageGetProjectionStatusRequest;
    type Response = wire::StorageGetProjectionStatusResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/GetProjectionStatus";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.source.clone())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (status, resolved_at) =
            super::views::projection_status(&context, Self::ID, request.name, request.revision, request.at, request.source).await?;
        super::native::read_response(&context, Self::Response { status, resolved_at: Some(resolved_at) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct ListProjectionRows;
impl ProtobufOperation for ListProjectionRows {
    type Request = wire::StorageListProjectionRowsRequest;
    type Response = wire::StorageListProjectionRowsResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/ListProjectionRows";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.source.clone())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (page, resolved_at, next_page_token) =
            super::lists::projections(&context, Self::ID, request.selection, request.source, request.at, request.page).await?;
        super::native::read_response(
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

impl ProtobufOperation for GetObject {
    type Request = wire::StorageGetObjectRequest;
    type Response = wire::StorageGetObjectResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/GetObject";
    const GRPC: Option<crate::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::snapshot::SavedSnapshots>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, super::object::source(request.schema.clone()))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (value, revision, resolved_at) = super::object::get(&context, Self::ID, request.schema, request.item, request.at).await?;
        super::native::read_response(
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
impl ProtobufOperation for SetObject {
    type Request = wire::StorageSetObjectRequest;
    type Response = wire::StorageSetObjectResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/SetObject";
    const GRPC: Option<crate::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, super::object::source(request.value.as_ref().and_then(|value| value.schema.clone())))
    }
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
        use wire::storage_set_object_response::{Applied, Conflict, Outcome};
        let outcome = match result {
            super::object::SetOutcome::Applied(value, revision, receipt) => Outcome::Applied(Applied {
                value: Some(value),
                revision,
                receipt: Some(receipt),
            }),
            super::object::SetOutcome::Conflict(current_revision) => Outcome::Conflict(Conflict {
                current_revision,
                action: "SetObject: get again and retry".into(),
            }),
        };
        super::native::read_response(&context, Self::Response { outcome: Some(outcome) })
    }
}

/* === Immutable Append === */

pub struct AppendRecords;
impl ProtobufOperation for AppendRecords {
    type Request = wire::StorageAppendRecordsRequest;
    type Response = wire::StorageAppendRecordsResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/AppendRecords";
    const GRPC: Option<crate::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::append::grant(Self::ID, request.batch.as_ref())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let receipt =
            super::append::append(&context, Self::ID, super::append::Kind::Records, request.mutation, request.scope, request.batch).await?;
        super::native::read_response(&context, Self::Response { receipt: Some(receipt) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
pub struct AppendSamples;
impl ProtobufOperation for AppendSamples {
    type Request = wire::StorageAppendSamplesRequest;
    type Response = wire::StorageAppendSamplesResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/AppendSamples";
    const GRPC: Option<crate::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::append::grant(Self::ID, request.batch.as_ref())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let receipt =
            super::append::append(&context, Self::ID, super::append::Kind::Samples, request.mutation, request.scope, request.batch).await?;
        super::native::read_response(&context, Self::Response { receipt: Some(receipt) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
pub struct AppendObjectRevisions;
impl ProtobufOperation for AppendObjectRevisions {
    type Request = wire::StorageAppendObjectRevisionsRequest;
    type Response = wire::StorageAppendObjectRevisionsResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/AppendObjectRevisions";
    const GRPC: Option<crate::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::append::grant(Self::ID, request.batch.as_ref())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let receipt =
            super::append::append(&context, Self::ID, super::append::Kind::Objects, request.mutation, request.scope, request.batch).await?;
        super::native::read_response(&context, Self::Response { receipt: Some(receipt) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct WatchRows;

impl ProtobufOperation for WatchRows {
    type Request = wire::StorageWatchRowsRequest;
    type Response = wire::StorageWatchRowsResponse;
    type Endpoints = crate::execution::operations::ProtobufOutput<wire::StorageWatchRowsResponse>;
    const ID: &'static str = "harmos.context.v1.Storage/WatchRows";

    const POLICY: crate::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::SavedSnapshots>()];

    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::pages::grant(Self::ID, request.source.clone())
    }
    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let context = &context;
        let frame = super::watch_rows::run(context, Self::ID, request.source, request.at, |frame| async move {
            let response = super::native::read_response(context, Self::response(frame))?;
            output.send(response).await.map_err(ServingError::Stream)
        })
        .await?;
        Ok(Self::response(frame))
    }
}
impl WatchRows {
    fn response(frame: super::watch_rows::Frame) -> wire::StorageWatchRowsResponse {
        use super::watch_rows::Event;
        use wire::storage_watch_rows_response::Event as Wire;
        wire::StorageWatchRowsResponse {
            resolved_at: Some(frame.at),
            event: Some(match frame.event {
                Event::Boundary => Wire::Boundary(harmos_protocol::storage::Empty {}),
                Event::Record(row) => Wire::Record(row),
                Event::Sample(row) => Wire::Sample(row),
                Event::Object(row) => Wire::Object(row),
                Event::Progress => Wire::DurableProgress(harmos_protocol::storage::Empty {}),
                Event::Resync(error) => Wire::ResyncRequired(error),
            }),
        }
    }
}

pub struct WatchFileBytes;
impl ProtobufOperation for WatchFileBytes {
    type Request = wire::StorageWatchFileBytesRequest;
    type Response = wire::StorageWatchFileBytesResponse;
    type Endpoints = crate::execution::operations::ProtobufOutput<wire::StorageWatchFileBytesResponse>;
    const ID: &'static str = "harmos.context.v1.Storage/WatchFileBytes";

    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::file_watches::grant(Self::ID, request.source.as_ref())
    }
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
impl ProtobufOperation for WatchFileProduction {
    type Request = wire::StorageWatchFileProductionRequest;
    type Response = wire::StorageWatchFileProductionResponse;
    type Endpoints = crate::execution::operations::ProtobufOutput<wire::StorageWatchFileProductionResponse>;
    const ID: &'static str = "harmos.context.v1.Storage/WatchFileProduction";

    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::transfers::grant(Self::ID, request.file.as_ref().map(|file| file.id.as_str()))
    }
    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let context = &context;
        let progress = super::file_watches::production(context, Self::ID, request.file, request.offset, |data| async move {
            super::file_watches::send(
                context,
                Self::ID,
                output,
                Self::Response {
                    event: Some(wire::storage_watch_file_production_response::Event::Data(data)),
                },
            )
            .await
        })
        .await?;
        let response = Self::Response {
            event: Some(wire::storage_watch_file_production_response::Event::Progress(progress)),
        };
        super::file_watches::send(context, Self::ID, output, response.clone()).await?;
        Ok(response)
    }
}

pub struct WatchDirectoryEntries;
impl ProtobufOperation for WatchDirectoryEntries {
    type Request = wire::StorageWatchDirectoryEntriesRequest;
    type Response = wire::StorageWatchDirectoryEntriesResponse;
    type Endpoints = crate::execution::operations::ProtobufOutput<wire::StorageWatchDirectoryEntriesResponse>;
    const ID: &'static str = "harmos.context.v1.Storage/WatchDirectoryEntries";

    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(_: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        Ok(None)
    }
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
impl ProtobufOperation for GetFileHash {
    type Request = wire::StorageGetFileHashRequest;
    type Response = wire::StorageGetFileHashResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/GetFileHash";

    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::SavedSnapshots>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::file_watches::grant(Self::ID, request.source.as_ref())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (hash, at) = super::file_access::hash(&context, Self::ID, request.source, request.range, request.at).await?;
        super::native::read_response(&context, Self::Response { hash: Some(hash), resolved_at: Some(at) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct CaptureFile;
impl ProtobufOperation for CaptureFile {
    type Request = wire::StorageCaptureFileRequest;
    type Response = wire::StorageCaptureFileResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/CaptureFile";

    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::SavedSnapshots>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::file_watches::grant(Self::ID, request.source.as_ref())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (file, receipt) =
            super::file_access::capture(&context, Self::ID, request.source, request.range, request.metadata, request.consistency).await?;
        super::native::read_response(&context, Self::Response { file: Some(file), receipt: Some(receipt) })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

pub struct ListDirectoryEntries;
impl ProtobufOperation for ListDirectoryEntries {
    type Request = wire::StorageListDirectoryEntriesRequest;
    type Response = wire::StorageListDirectoryEntriesResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Storage/ListDirectoryEntries";

    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>(), Resource::of::<super::SavedSnapshots>()];
    fn grant(_: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        Ok(None)
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let page = super::file_access::directory(&context, request.selection, request.at, request.page).await?;
        Ok(Self::Response {
            entries: page.entries,
            next_page_token: page.next_page_token,
            resolved_at: page.resolved_at,
        })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
