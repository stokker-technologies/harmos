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
    const ID: &'static str = "harmos.context.v1.storage.create_file_production";

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
    const ID: &'static str = "harmos.context.v1.storage.get_file_production";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];

    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        transfers::grant(Self::ID, request.file.as_ref().map(|value| value.id.as_str()))
    }

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            Self::grant(&request).map_err(|_| crate::Error::InvalidInput {
                field: "GetFileProduction",
                reason: "file identity is required",
            })?;
            transfers::admit(&context, Self::ID, request.file.as_ref().map(|value| value.id.as_str()).expect("validated file identity"))?;
            let status = transfers::status(&storage, transfers::required(request.file)?).await?;
            Ok(Self::Response {
                production: Some(status.progress.into()),
                writer_generation: status.writer_generation.unwrap_or(0),
                resolved_at: Some(transfers::cut(storage.get_saved_position())),
                accepted_length: status.accepted_length,
                durable_length: status.durable_length,
            })
        }
        .await;
        result.map_err(super::native::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl ProtobufOperation for AppendFileBytes {
    type Request = wire::StorageAppendFileBytesRequest;
    type Response = wire::StorageAppendFileBytesResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.storage.append_file_bytes";

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
    const ID: &'static str = "harmos.context.v1.storage.create_file_writer";

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
    const ID: &'static str = "harmos.context.v1.storage.publish_file_production";

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
    const ID: &'static str = "harmos.context.v1.storage.delete_file_production";

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
    const ID: &'static str = "harmos.context.v1.storage.get_file_bytes";

    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];

    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        transfers::grant(Self::ID, request.file.as_ref().and_then(|value| value.metadata.as_ref()).map(|value| value.id.as_str()))
    }

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
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
            let range = transfers::get(&storage, transfers::required(request.file)?, request.offset, request.max_bytes, request.at).await?;
            let mut response = Self::Response {
                data: range.data,
                total_length: range.total_length,
                sha256: range.sha256.to_vec(),
                resolved_at: Some(transfers::cut(range.resolved_at)),
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
    const ID: &'static str = "harmos.context.v1.storage.get_index_definition";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.source.clone())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, resolved_at) = super::definitions::get_index(&context, Self::ID, request.name, request.source).await?;
        Ok(Self::Response {
            definition: Some(definition),
            revision,
            resolved_at: Some(resolved_at),
        })
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
    const ID: &'static str = "harmos.context.v1.storage.set_index_definition";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.definition.as_ref().and_then(|definition| definition.source.clone()))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision) = super::definitions::set_index(
            &context,
            Self::ID,
            request.definition,
            request.expected_revision,
            request.create_if_absent,
            request.rebuild,
            request.work,
        )
        .await?;
        Ok(Self::Response { definition: Some(definition), revision })
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
    const ID: &'static str = "harmos.context.v1.storage.get_projection_definition";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.source.clone())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision, resolved_at) =
            super::definitions::get_projection(&context, Self::ID, request.name, request.source).await?;
        Ok(Self::Response {
            definition: Some(definition),
            revision,
            resolved_at: Some(resolved_at),
        })
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
    const ID: &'static str = "harmos.context.v1.storage.set_projection_definition";
    const GRPC: Option<crate::execution::operations::Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.definition.as_ref().and_then(|definition| definition.source.clone()))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let (definition, revision) = super::definitions::set_projection(
            &context,
            Self::ID,
            request.definition,
            request.expected_revision,
            request.create_if_absent,
            request.rebuild,
        )
        .await?;
        Ok(Self::Response { definition: Some(definition), revision })
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
