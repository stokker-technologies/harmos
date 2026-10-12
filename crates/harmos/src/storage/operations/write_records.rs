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
use crate::Identifier;

use crate::operations::{Context, Operation};
use crate::operations::{Grpc, OperationDeclaration, ReplaySafety, Visibility, WireOperation};

use crate::storage::StorageManager;
use crate::storage::{MutationId, RecordInput, StorageCursor};

use crate::{Error, Result};
use harmos_contract::plugin::Grant;

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(WriteRecordsRequest {
    mutation: crate::storage::codec::Nested<MutationId> => 1,
    records: crate::storage::codec::Repeated<RecordInput> => 2,
    source: crate::storage::codec::Id => 3,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteRecordsRequest {
    pub mutation: MutationId,
    pub records: Vec<RecordInput>,
    pub source: Identifier,
}

pub struct WriteRecords;

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for WriteRecords {
    type Request = WriteRecordsRequest;
    type Response = StorageCursor;
    const ID: &'static str = "storage.write_records";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    fn grant(request: &WriteRecordsRequest) -> Option<Grant> {
        Some(Grant::Source(request.source.as_str().to_owned().into()))
    }
}
impl WireOperation for WriteRecords {
    const VERSION: u32 = 1;
    const GRPC: Option<Grpc> = Some(super::grpc("WriteRecords"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
            Some(visitor.unary::<Self>())
        }
    }
}
impl crate::MemorySize for WriteRecordsRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<MutationId>()
            + self.records.memory_size()
            - std::mem::size_of::<Vec<RecordInput>>()
            + self.source.memory_size()
            - std::mem::size_of::<Identifier>()
    }
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = WriteRecords;
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
        request: &mut WriteRecordsRequest,
        _: &mut Self::Endpoints,
    ) -> Result<StorageCursor> {
        for record in &mut request.records {
            match &record.source {
                Some(source) if *source != request.source => {
                    return Err(Error::InvalidInput {
                        field: "storage record source",
                        reason: "a record names a source other than the granted tag",
                    });
                }
                _ => record.source = Some(request.source.clone()),
            }
        }
        let values = request.records.iter().map(RecordInput::borrowed);
        self.storage
            .admit_record_values(&request.mutation, values)
            .await
    }

    fn classify(error: &Error) -> crate::operations::Failure {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
