/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::any::TypeId;
use std::sync::Arc;
// Third Party Imports
// Local Imports
use crate::operations::{
    Context, Grpc, InputDeclaration, Operation, OperationDeclaration, ReplaySafety, StreamInput,
    Visibility, WireOperation,
};
use crate::schema::Described;
use crate::storage::files::FileBatch;
use crate::storage::{FileMetadata, FileReference, StorageManager};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct WriteFile;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for WriteFile {
    type Request = FileMetadata;
    type Response = FileReference;
    const ID: &'static str = "storage.write_file";
    const POLICY: crate::operations::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;

    fn grant(request: &FileMetadata) -> Option<harmos_contract::plugin::Grant> {
        Some(harmos_contract::plugin::Grant::of(
            Self::ID,
            request.id.as_str().to_owned(),
        ))
    }

    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (Some(TypeId::of::<FileBatch>()), None)
    }
}

impl InputDeclaration for WriteFile {
    type Item = FileBatch;
}

impl WireOperation for WriteFile {
    const VERSION: u32 = 1;
    const INPUT: Option<crate::schema::declaration::Definition> = Some(FileBatch::DEFINITION);
    const GRPC: Option<Grpc> = Some(super::grpc("WriteFile"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
            Some(visitor.upload::<Self>())
        }
    }
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = WriteFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamInput<FileBatch>;

    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        ctx: Self::Context<'a>,
        metadata: &mut FileMetadata,
        input: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileReference>> {
        let grant =
            harmos_contract::plugin::Grant::of(WriteFile::ID, metadata.id.as_str().to_owned());
        ctx.admit_stream_grant(&grant)?;
        self.storage.retry_transient_persistence();
        let mut file = self.storage.create_file(metadata.clone()).await?;
        let mut offset = 0u64;
        while let Some(batch) = input.recv().await {
            ctx.admit_stream_grant(&grant)?;
            if batch.offset != offset || batch.bytes.is_empty() {
                return Err(Error::InvalidInput {
                    field: "file batch",
                    reason: "nonempty batches must have contiguous byte offsets",
                });
            }
            let next = offset
                .checked_add(batch.bytes.len() as u64)
                .ok_or(Error::InvalidInput {
                    field: "file batch offset",
                    reason: "byte offset overflow",
                })?;
            file.write_all(&batch.bytes).await?;
            offset = next;
        }
        ctx.admit_stream_grant(&grant)?;
        let reference = file.finish().await?;
        self.storage.await_durable(&reference.position).await?;
        Ok(reference.into())
    }

    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
