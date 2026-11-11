/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Shared Built-In Clock Descriptors.

/* === Modules === */

/* === Re-Exports === */

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

/// Complete descriptor inventory for the two built-in clock operations.
pub fn catalog_types() -> prost_types::FileDescriptorSet {
    use prost::Message;
    prost_types::FileDescriptorSet::decode(include_bytes!("descriptors.bin").as_slice()).expect("generated clock descriptors")
}

/* === Private Functions === */

/* === Tests === */
