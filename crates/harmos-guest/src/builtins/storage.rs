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
use harmos_domain::operations::ReplaySafety;
use harmos_operations::storage::IO_POLICY;

/* === Constants === */

/* === Macros === */

product!(HashFile {
    source: Nested<FileSource> => 1,
    range: Nested<ByteRange> => 2,
});

/* === Type Aliases === */

/* === Structs === */

#[harmos_operations::execution::operation(harmos_path = harmos_operations::execution, codec = manual,
    id = "storage.hash_file",
    response = FileHash,
    policy = IO_POLICY,
    visibility = Public,
    replay = ReplaySafety::Idempotent,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "HashFile"),
    grant = Self::grant_for)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashFile {
    pub source: FileSource,
    pub range: ByteRange,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl HashFile {
    fn grant_for(request: &HashFile) -> Option<harmos_domain::plugin::Grant> {
        request.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, scope))
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
