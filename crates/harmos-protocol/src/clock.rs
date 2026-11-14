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

/// Complete descriptor inventory for the two built-in clock operations.
pub fn declarations_types() -> FileDescriptorSet {
    FileDescriptorSet::decode(include_bytes!("clock-descriptors.bin").as_slice()).expect("generated host clock descriptors")
}

/* === Private Functions === */

/* === Tests === */
