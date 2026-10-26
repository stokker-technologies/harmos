/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Lifecycle Actions And The Built-In Instance Operations Harmos Serves
// Over Its Own State. A Host Serves None Of The Instance Operations Until It
// Registers Them Explicitly.

/* === Modules === */

mod instance;
mod payloads;
mod plugin;

/* === Re-Exports === */

pub use instance::{Cancel, Status, Watch};
pub use payloads::{CancelRequest, InstanceState, StatusRequest, WatchRequest};
pub use plugin::*;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
