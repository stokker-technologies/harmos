/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod context;
mod instance;
mod publisher;

pub mod plugin;

/* === Re-Exports === */

pub use harmos_sdk::{Operation, Service};

pub use crate::export_plugin;
pub use context::Context;
pub use harmos_domain::Refusal as OperationRefusal;
pub use harmos_domain::{UNKNOWN, UNREADABLE};
pub use instance::OperationInstance;
pub use publisher::{PublishError, Publisher};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

pub const WIT: &str = include_str!("../../wit/harmos.wit");

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
