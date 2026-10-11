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
#[cfg(feature = "runtime")]
use std::sync::Arc;

// Third Party Imports

// Local Imports
#[cfg(feature = "runtime")]
use crate::operations::{Context, Operation};
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};
#[cfg(feature = "runtime")]
use crate::storage::StorageManager;
use crate::storage::{MutationId, StagedFile};
#[cfg(feature = "runtime")]
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */
crate::storage_codec::product!(DeleteFileRequest {
    mutation: crate::storage_codec::Nested<MutationId> => 1,
    file: crate::storage_codec::Nested<StagedFile> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteFileRequest {
    pub mutation: MutationId,
    pub file: StagedFile,
}

pub struct DeleteFile;

#[cfg(feature = "runtime")]
pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl crate::MemorySize for DeleteFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<MutationId>()
            + self.file.memory_size()
            - std::mem::size_of::<StagedFile>()
    }
}

impl OperationDeclaration for DeleteFile {
    type Request = DeleteFileRequest;
    type Response = StagedFile;
    const ID: &'static str = "storage.delete_file";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}

impl WireOperation for DeleteFile {
    const VERSION: u32 = 1;
}

#[cfg(feature = "runtime")]
impl<C: 'static> Operation for Handler<C> {
    type Declaration = DeleteFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    fn new(storage: Arc<StorageManager<C>>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut DeleteFileRequest,
        _: &mut Self::Endpoints,
    ) -> Result<StagedFile> {
        self.storage
            .delete_file_once(&request.mutation, &request.file)
            .await
    }

    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
