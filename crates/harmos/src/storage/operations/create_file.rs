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
use crate::storage::FileProducer;

use crate::storage::StorageManager;

use crate::{Error, Result};

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(CreateFileRequest {
    mutation: crate::storage::codec::Nested<crate::storage::MutationId> => 1,
    metadata: crate::storage::codec::Nested<crate::storage::FileMetadata> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateFileRequest {
    pub mutation: crate::storage::MutationId,
    pub metadata: crate::storage::FileMetadata,
}

pub struct CreateFile;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for CreateFile {
    type Request = CreateFileRequest;
    type Response = FileProducer;
    const ID: &'static str = "storage.create_file";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}
impl WireOperation for CreateFile {
    const VERSION: u32 = 1;
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = CreateFile;
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
        request: &mut CreateFileRequest,
        _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<FileProducer>> {
        let result: ::core::result::Result<FileProducer, Self::Error> = async move {
            self.storage
                .create_file_once(&request.mutation, &request.metadata)
                .await
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

impl crate::MemorySize for CreateFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<crate::storage::MutationId>()
            + self.metadata.memory_size()
            - std::mem::size_of::<crate::storage::FileMetadata>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
