/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Application-Owned File Reads and Hashes

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_operations::Failure;
use harmos_operations::execution::operations::{
    ExecutionContext, FailureKind, Grpc, OperationPolicy, ProtobufOperation, ReplaySafety, RetryPolicy, ServingError, Visibility,
};
use harmos_operations::storage::{self, ByteRange, FileSource, GetFileBytes, GetFileBytesRequest, GetFileHash, GetFileHashRequest};
use schematic_protocol::{FilesHashRequest, FilesHashResponse, FilesReadRequest, FilesReadResponse, files_hash_request::Source};

/* === Constants === */

const POLICY: OperationPolicy = OperationPolicy {
    retries: RetryPolicy::Never,
    ..storage::POLICY
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct FilesRead;
pub struct FilesHash;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for FilesHash {
    type Request = FilesHashRequest;
    type Response = FilesHashResponse;
    type Endpoints = ();
    const ID: &'static str = "schematic.v1.Files/Hash";
    const GRPC: Option<Grpc> = Some(Grpc::new("schematic.v1", "Files", "Hash"));
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;

    async fn run(context: ExecutionContext, request: FilesHashRequest, _: &()) -> Result<FilesHashResponse, ServingError> {
        let source = match request.source.ok_or_else(|| invalid("Hash File: Missing Source"))? {
            Source::File(file) => FileSource::Stored(
                file.reference
                    .ok_or_else(|| invalid("Hash File: Missing Reference"))?
                    .try_into()
                    .map_err(|_| invalid("Hash File: Invalid Reference"))?,
            ),
            Source::Path(path) if !path.is_empty() => FileSource::Local(path.into()),
            Source::Path(_) => return Err(invalid("Hash File: Empty Path")),
        };
        let request = GetFileHashRequest {
            source: Some(source),
            range: Some(ByteRange {
                offset: request.offset,
                limit: request.length,
            }),
            at: request.at,
        };
        let response = context.call_native::<GetFileHash>(request).await?;
        let hash = response.hash.as_ref().ok_or_else(|| invalid("Hash File: Missing Digest"))?;
        Ok(FilesHashResponse {
            read_bytes: hash.bytes_read,
            sha256: hash.sha256.to_vec(),
            resolved_at: response.resolved_at.clone(),
        })
    }
}

impl ProtobufOperation for FilesRead {
    type Request = FilesReadRequest;
    type Response = FilesReadResponse;
    type Endpoints = ();
    const ID: &'static str = "schematic.v1.Files/Read";
    const GRPC: Option<Grpc> = Some(Grpc::new("schematic.v1", "Files", "Read"));
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let file = request
            .file
            .and_then(|file| file.reference)
            .ok_or_else(|| invalid("Read File: Missing Reference"))?
            .try_into()
            .map_err(|_| invalid("Read File: Invalid Reference"))?;
        let offset = request.offset;
        let request = GetFileBytesRequest {
            file: Some(file),
            offset,
            max_bytes: request.max_bytes,
            at: request.at,
        };
        let response = context.call_native::<GetFileBytes>(request).await?;
        Ok(FilesReadResponse {
            offset,
            bytes: response.data.clone(),
            complete: offset.saturating_add(response.data.len() as u64) >= response.total_length,
            resolved_at: response.resolved_at.clone(),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(message: &str) -> ServingError {
    ServingError::Handler(Failure::new(FailureKind::Invalid, message))
}

/* === Tests === */
