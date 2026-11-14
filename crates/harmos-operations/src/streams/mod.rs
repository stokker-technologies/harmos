/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod native;

/* === Re-Exports === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use native::*;
#[derive(Clone, Debug, PartialEq)]
pub struct PublishedRow {
    pub payload: Vec<u8>,
    pub published_delta: Option<prost_types::Duration>,
}

impl crate::MemorySize for PublishedRow {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.payload.capacity())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PublishRequest {
    pub stream: String,
    pub sequence: Option<u64>,
    pub published_at: Option<prost_types::Timestamp>,
    pub context: Option<String>,
    pub payload: Vec<u8>,
    pub following: Vec<PublishedRow>,
}
impl crate::MemorySize for PublishRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.stream.capacity())
            .saturating_add(self.context.memory_size())
            .saturating_add(self.payload.capacity())
            .saturating_add(self.following.memory_size())
    }
}

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
