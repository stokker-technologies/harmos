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
    Context, Grpc, Operation, OperationDeclaration, OutputDeclaration, ReplaySafety, StreamOutput,
    Visibility, WireOperation,
};
use crate::schema::Described;
use crate::storage::StorageManager;
use crate::storage::config::IO_TIMEOUT;
use crate::storage::files::{FileBatch, FileCompletion, ReadFile as ReadRequest};
use crate::{Error, MemorySize, Result};

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
        let grant = harmos_contract::plugin::Grant::of(
            ReadFile::ID,
            request.file.metadata.id.as_str().to_owned(),
        );
        ctx.admit_stream_grant(&grant)?;
        let mut reader = self.storage.read_file(request.clone()).await?;
        while let Some(batch) = reader.next().await? {
            ctx.admit_stream_grant(&grant)?;
            let charge = batch.memory_size();
            tokio::time::timeout(IO_TIMEOUT, output.send_with(charge, || Ok(batch)))
                .await
                .map_err(|_| super::output_timeout())??;
        }
        Ok(reader.completion()?.into())
    }

    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */
/* === Tests === */
