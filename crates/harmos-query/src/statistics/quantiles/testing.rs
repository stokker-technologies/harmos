/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Numeric Spool Evidence For Test Consumers

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::mem;

// Third Party Imports

// Local Imports
use super::Quantiles;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Quantiles {
    pub fn size(&self) -> usize {
        mem::size_of::<Self>() + self.memory.capacity() * 8 + self.file.as_ref().map_or(0, |file| file.capacity())
    }
    pub fn spilled(&self) -> bool {
        self.file.is_some()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
