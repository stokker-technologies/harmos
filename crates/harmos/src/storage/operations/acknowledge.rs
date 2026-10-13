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
use crate::operations::{Grpc, OperationDeclaration, ReplaySafety, Visibility, WireOperation};
use crate::storage::StorageCursor;

use crate::storage::codec::{Id, Nested, product};
use crate::storage::{ReplicaHolder, StorageManager};

use crate::{Error, Result};
use crate::{Identifier, MemorySize};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Acknowledge;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcknowledgeRequest {
    pub replica: Identifier,
    pub cursor: StorageCursor,
}

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
product!(AcknowledgeRequest {
    replica: Id => 1,
    cursor: Nested<StorageCursor> => 2,
});

impl MemorySize for AcknowledgeRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.replica
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<Identifier>()),
            )
            .saturating_add(
                self.cursor
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<StorageCursor>()),
            )
    }
}

impl OperationDeclaration for Acknowledge {
    type Request = AcknowledgeRequest;
    type Response = StorageCursor;
    const ID: &'static str = "storage.acknowledge";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}

impl WireOperation for Acknowledge {
    const VERSION: u32 = 1;
    const GRPC: Option<Grpc> = Some(super::grpc("Acknowledge"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
            Some(visitor.unary::<Self>())
        }
    }
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = Acknowledge;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    fn new(storage: Arc<StorageManager<C>>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        context: Self::Context<'a>,
        request: &mut AcknowledgeRequest,
        _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageCursor>> {
        let result: ::core::result::Result<StorageCursor, Self::Error> = async move {
            let holder = holder(&context)?;
            self.storage
                .acknowledge(&request.replica, &holder, &request.cursor)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn holder(_context: &Context<'_>) -> Result<ReplicaHolder> {
    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    if let Some(scope) = _context.caller_scope() {
        return Ok(ReplicaHolder {
            owner: scope.owner().to_owned(),
            generation: scope.generation().get(),
        });
    }
    Err(Error::InvalidInput {
        field: "replica receipt",
        reason: "a replica receipt binds to its calling plugin",
    })
}

/* === Tests === */
