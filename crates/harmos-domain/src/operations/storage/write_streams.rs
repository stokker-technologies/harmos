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

// Third Party Imports

// Local Imports
use super::IO_POLICY;
use crate::Identifier;
use crate::MemorySize;
use crate::model::storage::codec::{Id, Optional, product};
use crate::model::storage::{DataBatch, WriteCompletion, WriteReceipt};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(WriteStreamsRequest {
    scope: Optional<Id> => 1,
});

pub struct WriteStreams;

impl crate::operations::OperationDeclaration for WriteStreams {
    type Request = WriteStreamsRequest;
    type Response = WriteCompletion;
    type Input = crate::operations::Stream<DataBatch>;
    type Output = crate::operations::Stream<WriteReceipt>;
    const ID: &'static str = "storage_write_streams";
    const POLICY: crate::operations::OperationPolicy = IO_POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Unknown;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.storage.v1", "Storage", "WriteStreams"));
}

impl crate::operations::RequestFor for WriteStreamsRequest {
    type Operation = WriteStreams;
}

/* === Type Aliases === */

/* === Structs === */

// Admit Streamed Arrow Batches Of Stream Rows Into One Optional Scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteStreamsRequest {
    pub scope: Option<Identifier>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for WriteStreamsRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.scope.memory_size().saturating_sub(std::mem::size_of_val(&self.scope)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
