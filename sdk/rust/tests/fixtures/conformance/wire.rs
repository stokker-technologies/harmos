/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Generated Conformance Messages and Their Descriptor Closure.

/* === Modules === */

include!("generated.rs");

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost::Message;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn types() -> prost_types::FileDescriptorSet {
    prost_types::FileDescriptorSet::decode(include_bytes!("descriptors.bin").as_slice()).expect("generated conformance descriptors decode")
}

/* === Private Functions === */

/* === Tests === */
