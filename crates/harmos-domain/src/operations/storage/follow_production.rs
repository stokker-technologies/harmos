/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// FollowProductionRequest Declaration.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::IO_POLICY;
use crate::MemorySize;
use crate::model::storage::codec::{Nested, Unsigned, product};
use crate::model::storage::files::{FileBatch, FileProgress, StagedFile};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(FollowProductionRequest {
    file: Nested<StagedFile> => 1,
    offset: Unsigned => 2,
});

pub struct FollowProduction;

impl crate::operations::OperationDeclaration for FollowProduction {
    type Request = FollowProductionRequest;
    type Response = FileProgress;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::Stream<FileBatch>;
    const ID: &'static str = "storage_follow_production";
    const POLICY: crate::operations::OperationPolicy = IO_POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Unknown;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.storage.v1", "Storage", "FollowProduction"));
    fn grant(request: &Self::Request) -> Option<crate::plugin::Grant> {
        { Some(crate::plugin::Grant::of("storage.follow_production", request.file.id.as_str().to_owned())) }
    }
}

impl crate::operations::RequestFor for FollowProductionRequest {
    type Operation = FollowProduction;
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowProductionRequest {
    pub file: StagedFile,
    pub offset: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FollowProductionRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.file.memory_size().saturating_sub(std::mem::size_of_val(&self.file)))
            .saturating_add(self.offset.memory_size().saturating_sub(std::mem::size_of_val(&self.offset)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
