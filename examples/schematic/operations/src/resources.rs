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

pub struct AnalysisResources {
    pub(crate) analysis: tokio::sync::Semaphore,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Default for AnalysisResources {
    fn default() -> Self {
        Self { analysis: tokio::sync::Semaphore::new(1) }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
