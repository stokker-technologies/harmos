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
use crate::model::storage::codec::{Nested, product};
use crate::model::storage::{DirectoryCompletion, DirectoryEntry, DirectorySelection};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(ListDirectoryRequest { selection: Nested<DirectorySelection> => 1 });
pub struct ListDirectory;

impl crate::operations::OperationDeclaration for ListDirectory {
    type Request = ListDirectoryRequest;
    type Response = DirectoryCompletion;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::Stream<DirectoryEntry>;
    const ID: &'static str = "storage_list_directory";
    const POLICY: crate::operations::OperationPolicy = IO_POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Idempotent;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.storage.v1", "Storage", "ListDirectory"));
}

impl crate::operations::RequestFor for ListDirectoryRequest {
    type Operation = ListDirectory;
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListDirectoryRequest {
    pub selection: DirectorySelection,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for ListDirectoryRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
