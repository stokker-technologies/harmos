/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod analyze;
mod list_directories;
mod propose;
mod validate;

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
#[harmos::catalog]
pub(crate) enum Operations {
    Validate(validate::ValidateSchematic),
    Route(propose::ProposeRoute),
    Analyze(analyze::AnalyzeSchematic),
    ListDirectories(list_directories::ListDirectories),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
