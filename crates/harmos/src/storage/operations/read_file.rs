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
use std::num::NonZeroUsize;
use std::sync::Arc;
// Third Party Imports
// Local Imports
use crate::operations::{
    Context, Grpc, Operation, OperationDeclaration, OutputDeclaration, ReplaySafety, StreamOutput,
    Visibility, WireOperation,
};
use crate::schema::Described;
use crate::storage::StorageManager;
use crate::storage::files::{FileBatch, FileCompletion, ReadFile as ReadRequest};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct ReadFile;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for ReadFile {
    type Request = ReadRequest;
    type Response = FileCompletion;
    const ID: &'static str = "storage.read_file";
    const POLICY: crate::operations::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;

    fn grant(request: &ReadRequest) -> Option<harmos_contract::plugin::Grant> {
        Some(harmos_contract::plugin::Grant::of(
            Self::ID,
            request.file.metadata.id.as_str().to_owned(),
        ))
    }
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<FileBatch>()))
    }
}
impl OutputDeclaration for ReadFile {
    type Item = FileBatch;
}
impl WireOperation for ReadFile {
    const VERSION: u32 = 1;
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(FileBatch::DEFINITION);
    const GRPC: Option<Grpc> = Some(super::grpc("ReadFile"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> { Some(visitor.download::<Self>()) }
    }
}
impl<C: 'static> Operation for Handler<C> {
    type Declaration = ReadFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<FileBatch>;

    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        ctx: Self::Context<'a>,
        request: &mut ReadRequest,
        output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileCompletion>> {
        let end = request.range.end(request.file.length)?;
        let grant = harmos_contract::plugin::Grant::of(
            ReadFile::ID,
            request.file.metadata.id.as_str().to_owned(),
        );
        ctx.admit_stream_grant(&grant)?;
        let mut reader = self
            .storage
            .read_file(&request.file)
            .await?
            .range(request.range)?;
        let mut bytes = 0u64;
        loop {
            ctx.admit_stream_grant(&grant)?;
            let offset = reader.offset();
            if offset == end {
                reader.next_chunk(NonZeroUsize::new(1).unwrap()).await?;
                break;
            }
            let ceiling = self.storage.inner.settings.response.batch_bytes.get();
            let overhead = 2 + varint_bytes(offset) + varint_bytes(ceiling as u64);
            let maximum = ceiling
                .checked_sub(overhead)
                .and_then(NonZeroUsize::new)
                .ok_or(Error::ResourceExhausted {
                    resource: "file response batch bytes",
                })?;
            let _delivery = reader.reserve_delivery(maximum).await?;
            let Some(chunk) = reader.next_chunk(maximum).await? else {
                break;
            };
            bytes = bytes
                .checked_add(chunk.len() as u64)
                .ok_or(Error::InvalidInput {
                    field: "file read",
                    reason: "returned byte count overflow",
                })?;
            let resident = maximum
                .get()
                .checked_add(std::mem::size_of::<FileBatch>())
                .and_then(NonZeroUsize::new)
                .ok_or(Error::ResourceExhausted {
                    resource: "file response batch memory",
                })?;
            let batch = chunk
                .into_query_value()
                .try_map(resident, |bytes, _| Ok(FileBatch { offset, bytes }))?;
            tokio::time::timeout(
                self.storage.inner.settings.io_timeout,
                output.send_owned(batch.try_into_response()?),
            )
            .await
            .map_err(|_| super::output_timeout())??;
        }
        Ok(FileCompletion { bytes }.into())
    }
    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn varint_bytes(mut value: u64) -> usize {
    let mut bytes = 1;
    while value >= 128 {
        value >>= 7;
        bytes += 1;
    }
    bytes
}

/* === Tests === */
