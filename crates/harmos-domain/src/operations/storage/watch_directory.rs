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
use crate::MemorySize;
use crate::model::storage::codec::{Flag, Nested, product};
use crate::model::storage::{DirectoryCompletion, DirectoryEvent, DirectorySelection};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(WatchDirectoryRequest { selection: Nested<DirectorySelection> => 1, include_existing: Flag => 2 });
pub struct WatchDirectory;

impl crate::operations::OperationDeclaration for WatchDirectory {
    type Request = WatchDirectoryRequest;
    type Response = DirectoryCompletion;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::Stream<DirectoryEvent>;
    const ID: &'static str = "storage_watch_directory";
    const POLICY: crate::operations::OperationPolicy = IO_POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Idempotent;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.storage.v1", "Storage", "WatchDirectory"));
}

impl crate::operations::RequestFor for WatchDirectoryRequest {
    type Operation = WatchDirectory;
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchDirectoryRequest {
    pub selection: DirectorySelection,
    pub include_existing: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for WatchDirectoryRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
