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

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::{Context, Operation};
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};

use crate::storage::StorageManager;
use crate::storage::{MutationId, StagedFile};

use crate::{Error, Result};

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(DeleteFileRequest {
    mutation: crate::storage::codec::Nested<MutationId> => 1,
    file: crate::storage::codec::Nested<StagedFile> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteFileRequest {
    pub mutation: MutationId,
    pub file: StagedFile,
}

pub struct DeleteFile;

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

impl<C: 'static> Operation for Handler<C> {
    type Declaration = DeleteFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut DeleteFileRequest,
        _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StagedFile>> {
        let result: ::core::result::Result<StagedFile, Self::Error> = async move {
            self.storage
                .delete_file_once(&request.mutation, &request.file)
                .await
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
