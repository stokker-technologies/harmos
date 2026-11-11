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
use harmos_domain::MemorySize;
use harmos_domain::model::storage::codec::{Nested, product};
use harmos_domain::model::storage::files::{ByteRange, FileHash, FileSource};
use harmos_operations::operations::ReplaySafety;
use harmos_operations::storage::IO_POLICY;

/* === Constants === */

/* === Macros === */

product!(HashFile {
    source: Nested<FileSource> => 1,
    range: Nested<ByteRange> => 2,
});

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_operations::execution::Request)]
#[harmos_path(harmos_operations::execution)]
#[request(response = FileHash)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashFile {
    pub source: FileSource,
    pub range: ByteRange,
}

pub static HASH_FILE_DESCRIPTION: harmos_operations::execution::operations::WireDeclaration =
    harmos_operations::execution::operations::WireDeclaration::request::<HashFile>("storage.hash_file", IO_POLICY)
        .visibility(harmos_operations::execution::operations::Visibility::Public)
        .replay(ReplaySafety::Idempotent)
        .grpc(harmos_operations::execution::operations::Grpc::new("harmos.storage.v1", "Storage", "HashFile"));

pub const HASH_FILE_OPERATION: harmos_operations::execution::operations::OperationSpec<HashFile> =
    harmos_operations::execution::operations::OperationSpec::<HashFile>::new(&HASH_FILE_DESCRIPTION).grant(HashFile::grant_for);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl HashFile {
    fn grant_for(request: &HashFile) -> Option<harmos_operations::plugin::Grant> {
        request.source.grant_scope().map(|scope| harmos_operations::plugin::Grant::of(HASH_FILE_OPERATION.entry().id, scope))
    }
}

impl MemorySize for HashFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.range.memory_size().saturating_sub(std::mem::size_of_val(&self.range)))
    }
}

/* === Public Functions === */

/* === Private Functions === */
