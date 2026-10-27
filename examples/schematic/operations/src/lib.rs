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

pub mod directories;
pub mod schematic;
pub mod transfer;

#[cfg(feature = "runtime")]
mod error;

#[cfg(feature = "runtime")]
pub mod resources;

/* === Re-Exports === */

#[cfg(feature = "runtime")]
pub use error::Error;

pub use directories::list_directories::ListDirectories;
pub use schematic::analyze::AnalyzeSchematic;
pub use schematic::propose::ProposeRoute;
pub use schematic::streaming::{AnalyzeExchange, AnalyzeUpload};
pub use schematic::validate::ValidateSchematic;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

pub const POLICY: harmos::operations::OperationPolicy = harmos::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    retries: harmos::operations::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[harmos::catalog]
pub enum Operations {
    Validate(ValidateSchematic),
    Route(ProposeRoute),
    Analyze(AnalyzeSchematic),
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
