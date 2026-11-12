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
use super::transfers;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility, access::GrantAccess};
use crate::operations::{FailureKind, ReplaySafety};
use harmos_protocol::v1 as wire;
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
    const ID: &'static str = "harmos.v1.storage.create_file_production";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1", "Storage", "CreateFileProduction"));
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
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}

impl ProtobufOperation for GetFileProduction {
    type Request = wire::StorageGetFileProductionRequest;
    type Response = wire::StorageGetFileProductionResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.v1.storage.get_file_production";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1", "Storage", "GetFileProduction"));
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
            let (production, writer_generation, resolved_at) = transfers::status(&storage, transfers::required(request.file)?).await?;
            Ok(Self::Response {
                production: Some(production),
                writer_generation,
                resolved_at: Some(resolved_at),
            })
        }
        .await;
        result.map_err(super::native::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}

impl ProtobufOperation for AppendFileBytes {
    type Request = wire::StorageAppendFileBytesRequest;
    type Response = wire::StorageAppendFileBytesResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.v1.storage.append_file_bytes";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1", "Storage", "AppendFileBytes"));
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
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}

impl ProtobufOperation for CreateFileWriter {
    type Request = wire::StorageCreateFileWriterRequest;
    type Response = wire::StorageCreateFileWriterResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.v1.storage.create_file_writer";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1", "Storage", "CreateFileWriter"));
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
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}

impl ProtobufOperation for PublishFileProduction {
    type Request = wire::StoragePublishFileProductionRequest;
    type Response = wire::StoragePublishFileProductionResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.v1.storage.publish_file_production";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1", "Storage", "PublishFileProduction"));
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
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}

impl ProtobufOperation for DeleteFileProduction {
    type Request = wire::StorageDeleteFileProductionRequest;
    type Response = wire::StorageDeleteFileProductionResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.v1.storage.delete_file_production";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1", "Storage", "DeleteFileProduction"));
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
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}

impl ProtobufOperation for GetFileBytes {
    type Request = wire::StorageGetFileBytesRequest;
    type Response = wire::StorageGetFileBytesResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.v1.storage.get_file_bytes";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.v1", "Storage", "GetFileBytes"));
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
            Ok(Self::Response {
                data: range.data,
                total_length: range.total_length,
                sha256: range.sha256.to_vec(),
                resolved_at: Some(transfers::cut(range.resolved_at)),
            })
        }
        .await;
        result.map_err(super::native::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
