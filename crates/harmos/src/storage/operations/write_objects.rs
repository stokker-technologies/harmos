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
use crate::storage::{MutationId, ObjectReceipt, ObjectValue};

use crate::{Error, Result};

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(WriteObjectsRequest {
    mutation: crate::storage::codec::Nested<MutationId> => 1,
    objects: crate::storage::codec::Repeated<ObjectValue> => 2,
});

crate::storage::codec::product!(WriteObjectsResponse {
    objects: crate::storage::codec::Repeated<ObjectReceipt> => 1,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteObjectsRequest {
    pub mutation: MutationId,
    pub objects: Vec<ObjectValue>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteObjectsResponse {
    pub objects: Vec<ObjectReceipt>,
}
pub struct WriteObjects;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for WriteObjects {
    type Request = WriteObjectsRequest;
    type Response = WriteObjectsResponse;
    const ID: &'static str = "storage.write_objects";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}
impl WireOperation for WriteObjects {
    const VERSION: u32 = 1;
}
impl crate::MemorySize for WriteObjectsResponse {
    fn memory_size(&self) -> usize {
        self.objects.memory_size()
    }
}
impl crate::MemorySize for WriteObjectsRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<MutationId>()
            + self.objects.memory_size()
            - std::mem::size_of::<Vec<ObjectValue>>()
    }
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = WriteObjects;
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
        request: &mut WriteObjectsRequest,
        _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<WriteObjectsResponse>> {
        let result: ::core::result::Result<WriteObjectsResponse, Self::Error> = async move {
            let objects = self
                .storage
                .write_objects_once(&request.mutation, &request.objects)
                .await?;
            Ok(WriteObjectsResponse { objects })
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
