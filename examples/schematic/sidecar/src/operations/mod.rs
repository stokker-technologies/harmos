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
mod streaming;
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

#[harmos_domain::catalog]
pub(crate) enum Operations {
    Validate(validate::Validating),
    Route(propose::Routing),
    Analyze(analyze::Analyzing),
    ListDirectories(list_directories::Listing),
    AnalyzeUpload(streaming::Uploading),
    AnalyzeExchange(streaming::Exchanging),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
