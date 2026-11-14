/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Application-Owned Resumable Upload Operations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_operations::execution::operations::{
    ExecutionContext, FailureKind, Grpc, OperationPolicy, ProtobufOperation, ReplaySafety, RetryPolicy, ServingError, Visibility,
};
use harmos_operations::model::storage::FilePhase;
use harmos_operations::storage::{
    self, AppendFileBytes, AppendFileBytesRequest, CreateFileProduction, CreateFileProductionRequest, CreateFileWriter,
    CreateFileWriterRequest, DeleteFileProduction, DeleteFileProductionRequest, FileSession, GetFileProduction, GetFileProductionRequest,
    Info, InfoRequest, MutationId, PublishFileProduction, PublishFileProductionRequest, StagedFile,
};
use harmos_operations::{Failure, Identifier};
use harmos_protocol::v1::{File, Upload as UploadProgress, UploadState};
use schematic_protocol::{
    UploadMutation, UploadWindow, UploadsAppendRequest, UploadsAppendResponse, UploadsCreateRequest, UploadsCreateResponse,
    UploadsDeleteRequest, UploadsDeleteResponse, UploadsGetRequest, UploadsGetResponse, UploadsPublishRequest, UploadsPublishResponse,
    UploadsResumeRequest, UploadsResumeResponse,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct UploadsCreate;
pub struct UploadsGet;
pub struct UploadsResume;
pub struct UploadsAppend;
pub struct UploadsPublish;
pub struct UploadsDelete;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for UploadsCreate {
    type Request = UploadsCreateRequest;
    type Response = UploadsCreateResponse;
    type Endpoints = ();
    const ID: &'static str = "schematic.v1.Uploads/Create";
    const GRPC: Option<Grpc> = Some(Grpc::new("schematic.v1", "Uploads", "Create"));
    const POLICY: OperationPolicy = OperationPolicy {
        retries: RetryPolicy::Never,
        ..storage::POLICY
    };
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let metadata = request
            .metadata
            .ok_or_else(|| invalid("Create Upload: Missing Metadata"))?
            .try_into()
            .map_err(|_| invalid("Create Upload: Invalid Metadata"))?;
        let request = CreateFileProductionRequest {
            metadata: Some(metadata),
            mutation: Some(mutation(request.mutation, "Create Upload")?),
        };
        let response = context.call_native::<CreateFileProduction>(request).await?;
        let production = response.production.clone().ok_or_else(|| invalid("Create Upload: Missing Writer"))?;
        Ok(UploadsCreateResponse {
            upload: Some(UploadProgress {
                id: production.session.id.to_string(),
                history: Some(production.session.history.into()),
                writer_generation: production.session.generation,
                accepted_length: production.length,
                durable_length: 0,
                state: UploadState::Producing as i32,
            }),
            receipt: response.receipt.clone(),
        })
    }
}

impl ProtobufOperation for UploadsGet {
    type Request = UploadsGetRequest;
    type Response = UploadsGetResponse;
    type Endpoints = ();
    const ID: &'static str = "schematic.v1.Uploads/Get";
    const GRPC: Option<Grpc> = Some(Grpc::new("schematic.v1", "Uploads", "Get"));
    const POLICY: OperationPolicy = OperationPolicy {
        retries: RetryPolicy::Never,
        ..storage::POLICY
    };
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let Some(upload) = request.upload else {
            let response = context.call_native::<Info>(InfoRequest {}).await?;
            let mutations = response.mutations.clone().map(|window| UploadWindow {
                history: Some(window.history.into()),
                id: window.id.to_string(),
                expires_at: Some(window.expires_at.into()),
            });
            return Ok(UploadsGetResponse {
                upload: None,
                mutations,
                chunk_bytes: response.file_batch_bytes,
            });
        };
        let session = session(Some(upload), "Get Upload")?;
        let file = StagedFile {
            history: session.history.clone(),
            id: session.id.clone(),
        };
        let response = context.call_native::<GetFileProduction>(GetFileProductionRequest { file: Some(file), at: None }).await?;
        let production = response.production.ok_or_else(|| invalid("Get Upload: Missing Progress"))?;
        let state = match production.phase {
            FilePhase::Producing => UploadState::Producing,
            FilePhase::Finished => UploadState::Finished,
            FilePhase::Abandoned => UploadState::Abandoned,
        };
        Ok(UploadsGetResponse {
            upload: Some(UploadProgress {
                id: session.id.to_string(),
                history: Some(session.history.into()),
                writer_generation: response.writer_generation,
                accepted_length: response.accepted_length,
                durable_length: response.durable_length,
                state: state as i32,
            }),
            mutations: None,
            chunk_bytes: 0,
        })
    }
}

impl ProtobufOperation for UploadsResume {
    type Request = UploadsResumeRequest;
    type Response = UploadsResumeResponse;
    type Endpoints = ();
    const ID: &'static str = "schematic.v1.Uploads/Resume";
    const GRPC: Option<Grpc> = Some(Grpc::new("schematic.v1", "Uploads", "Resume"));
    const POLICY: OperationPolicy = OperationPolicy {
        retries: RetryPolicy::Never,
        ..storage::POLICY
    };
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let request = CreateFileWriterRequest {
            session: Some(session(request.upload, "Resume Upload")?),
            mutation: Some(mutation(request.mutation, "Resume Upload")?),
        };
        let response = context.call_native::<CreateFileWriter>(request).await?;
        let production = response.production.clone().ok_or_else(|| invalid("Resume Upload: Missing Writer"))?;
        Ok(UploadsResumeResponse {
            writer_generation: production.session.generation,
            accepted_length: production.length,
            receipt: response.receipt.clone(),
        })
    }
}

impl ProtobufOperation for UploadsAppend {
    type Request = UploadsAppendRequest;
    type Response = UploadsAppendResponse;
    type Endpoints = ();
    const ID: &'static str = "schematic.v1.Uploads/Append";
    const GRPC: Option<Grpc> = Some(Grpc::new("schematic.v1", "Uploads", "Append"));
    const POLICY: OperationPolicy = OperationPolicy {
        retries: RetryPolicy::Never,
        ..storage::POLICY
    };
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let request = AppendFileBytesRequest {
            session: Some(session(request.upload, "Append Upload")?),
            offset: request.offset,
            data: request.bytes,
            mutation: Some(mutation(request.mutation, "Append Upload")?),
        };
        let response = context.call_native::<AppendFileBytes>(request).await?;
        let production = response.production.clone().ok_or_else(|| invalid("Append Upload: Missing Writer"))?;
        Ok(UploadsAppendResponse {
            accepted_length: production.length,
            receipt: response.receipt.clone(),
        })
    }
}

impl ProtobufOperation for UploadsPublish {
    type Request = UploadsPublishRequest;
    type Response = UploadsPublishResponse;
    type Endpoints = ();
    const ID: &'static str = "schematic.v1.Uploads/Publish";
    const GRPC: Option<Grpc> = Some(Grpc::new("schematic.v1", "Uploads", "Publish"));
    const POLICY: OperationPolicy = OperationPolicy {
        retries: RetryPolicy::Never,
        ..storage::POLICY
    };
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let request = PublishFileProductionRequest {
            session: Some(session(request.upload, "Publish Upload")?),
            length: request.length,
            sha256: request.sha256,
            mutation: Some(mutation(request.mutation, "Publish Upload")?),
        };
        let response = context.call_native::<PublishFileProduction>(request).await?;
        let reference = response.file.clone().ok_or_else(|| invalid("Publish Upload: Missing File"))?;
        Ok(UploadsPublishResponse {
            file: Some(File { reference: Some(reference.into()) }),
            receipt: response.receipt.clone(),
        })
    }
}

impl ProtobufOperation for UploadsDelete {
    type Request = UploadsDeleteRequest;
    type Response = UploadsDeleteResponse;
    type Endpoints = ();
    const ID: &'static str = "schematic.v1.Uploads/Delete";
    const GRPC: Option<Grpc> = Some(Grpc::new("schematic.v1", "Uploads", "Delete"));
    const POLICY: OperationPolicy = OperationPolicy {
        retries: RetryPolicy::Never,
        ..storage::POLICY
    };
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let request = DeleteFileProductionRequest {
            session: Some(session(request.upload, "Delete Upload")?),
            mutation: Some(mutation(request.mutation, "Delete Upload")?),
        };
        let response = context.call_native::<DeleteFileProduction>(request).await?;
        let production = response.production.ok_or_else(|| invalid("Delete Upload: Missing Progress"))?;
        let state = match production.phase {
            FilePhase::Producing => UploadState::Producing,
            FilePhase::Finished => UploadState::Finished,
            FilePhase::Abandoned => UploadState::Abandoned,
        };
        Ok(UploadsDeleteResponse {
            state: state as i32,
            receipt: response.receipt.clone(),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn mutation(value: Option<UploadMutation>, action: &str) -> Result<MutationId, ServingError> {
    let value = value.ok_or_else(|| invalid(&format!("{action}: Missing Mutation")))?;
    let window = Identifier::new(value.window).map_err(|_| invalid(&format!("{action}: Invalid Mutation Window")))?;
    let key = Identifier::new(value.key).map_err(|_| invalid(&format!("{action}: Invalid Mutation Key")))?;
    Ok(MutationId { window, key })
}

fn session(value: Option<UploadProgress>, action: &str) -> Result<FileSession, ServingError> {
    let value = value.ok_or_else(|| invalid(&format!("{action}: Missing Upload")))?;
    let history = value
        .history
        .ok_or_else(|| invalid(&format!("{action}: Missing History")))?
        .try_into()
        .map_err(|_| invalid(&format!("{action}: Invalid History")))?;
    let id = Identifier::new(value.id).map_err(|_| invalid(&format!("{action}: Invalid Upload Identity")))?;
    Ok(FileSession {
        history,
        id,
        generation: value.writer_generation,
    })
}

fn invalid(message: &str) -> ServingError {
    ServingError::Handler(Failure::new(FailureKind::Invalid, message))
}

/* === Tests === */
