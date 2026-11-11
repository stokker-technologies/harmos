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

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug)]
pub struct SchemaSettings {
    pub preparation_bytes: std::num::NonZeroUsize,
    pub preparation_work: std::num::NonZeroUsize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Default for SchemaSettings {
    fn default() -> Self {
        Self {
            // Complete Checkpoint Schemas Fit Below 128 KiB; Retain Headroom For Larger Catalogs.
            preparation_bytes: std::num::NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
            preparation_work: std::num::NonZeroUsize::new(1024 * 1024).unwrap(),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
