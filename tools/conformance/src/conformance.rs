/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Expose the Reference Conformance Messages

/* === Modules === */

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdk/rust/tests/fixtures/conformance/generated.rs"));

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost::Message;
use prost_types::FileDescriptorSet;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn types() -> FileDescriptorSet {
    FileDescriptorSet::decode(
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdk/rust/tests/fixtures/conformance/descriptors.bin")).as_slice(),
    )
    .expect("generated conformance descriptors decode")
}

/* === Private Functions === */

/* === Tests === */
