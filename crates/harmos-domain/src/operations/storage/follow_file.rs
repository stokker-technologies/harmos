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
use crate::model::storage::files::{FileCompletion, FileEvent, FileSource, FollowStart};
use crate::operations::Visibility;

/* === Constants === */

/* === Macros === */

product!(FollowFileRequest {
    source: Nested<FileSource> => 1,
    start: Nested<FollowStart> => 2,
});

pub struct FollowFile;

impl crate::operations::OperationDeclaration for FollowFile {
    type Request = FollowFileRequest;
    type Response = FileCompletion;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::Stream<FileEvent>;
    const ID: &'static str = "storage_follow_file";
    const POLICY: crate::operations::OperationPolicy = IO_POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.storage.v1", "Storage", "FollowFile"));
    fn grant(request: &Self::Request) -> Option<crate::plugin::Grant> {
        request.source.grant_scope().map(|scope| crate::plugin::Grant::of(Self::ID, scope))
    }
}

impl crate::operations::RequestFor for FollowFileRequest {
    type Operation = FollowFile;
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowFileRequest {
    pub source: FileSource,
    pub start: FollowStart,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FollowFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.start.memory_size().saturating_sub(std::mem::size_of_val(&self.start)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
