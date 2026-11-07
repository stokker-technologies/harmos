/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Closed Catalogs of Schematic Operations: the Portable Catalog Every
// Delivery Serves, and the Native Catalog That Adds Host Filesystem Access.

/* === Modules === */

pub mod directories;
pub mod schematic;
pub mod transfer;

#[cfg(feature = "runtime")]
mod error;

/* === Re-Exports === */

#[cfg(feature = "runtime")]
pub use error::Error;

pub use directories::list_directories::ListDirectories;
pub use schematic::analyze::AnalyzeSchematic;
pub use schematic::exchange::AnalyzeExchange;
pub use schematic::propose::ProposeRoute;
pub use schematic::upload::AnalyzeUpload;
pub use schematic::validate::ValidateSchematic;

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
pub enum Operations {
    ValidateSchematic(ValidateSchematic),
    ProposeRoute(ProposeRoute),
    AnalyzeSchematic(AnalyzeSchematic),
}

#[harmos::catalog]
pub enum Native {
    #[nested]
    Shared(Operations),

    ListDirectories(ListDirectories),
    AnalyzeUpload(AnalyzeUpload),
    AnalyzeExchange(AnalyzeExchange),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
