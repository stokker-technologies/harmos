/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Closed Catalogs Of Schematic Operations: The Portable Catalog Every
// Delivery Serves, And The Native Catalog That Adds Host Filesystem Access.

/* === Modules === */

mod common;
pub mod directories;
pub mod schematic;

/* === Re-Exports === */

pub use common::POLICY;
pub use directories::ListDirectories;
pub use schematic::streaming::{AnalyzeExchange, AnalyzeUpload};
pub use schematic::{AnalyzeSchematic, ProposeRoute, ValidateSchematic};

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
pub enum Operations {
    Validate(ValidateSchematic),
    Route(ProposeRoute),
    Analyze(AnalyzeSchematic),
}

#[harmos_domain::catalog]
pub enum Native {
    #[nested]
    Shared(Operations),

    ListDirectories(ListDirectories),
    AnalyzeUpload(AnalyzeUpload),
    AnalyzeExchange(AnalyzeExchange),
}

#[harmos_domain::catalog]
pub enum Requires {
    Publish(harmos_domain::operations::streams::Publish),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
