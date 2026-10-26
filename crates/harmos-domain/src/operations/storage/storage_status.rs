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
use super::POLICY;
use crate::MemorySize;
use crate::model::storage::StorageInfo;
use crate::model::storage::codec::product;
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(StorageStatusRequest {});

pub struct StorageStatus;

impl crate::operations::OperationDeclaration for StorageStatus {
    type Request = StorageStatusRequest;
    type Response = StorageInfo;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "storage_status";
    const POLICY: crate::operations::OperationPolicy = POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Host;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Idempotent;
}

impl crate::operations::RequestFor for StorageStatusRequest {
    type Operation = StorageStatus;
}

/* === Type Aliases === */

/* === Structs === */

// Read The Storage Frontiers, Mutation Window And File Batch Size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StorageStatusRequest {}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for StorageStatusRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
