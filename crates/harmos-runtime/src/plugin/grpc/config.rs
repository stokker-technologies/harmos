/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// gRPC Plugin Placement Policy

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::Duration;

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Placement Policy Uses The Same Clock As The Owning Host.
#[derive(Clone, Debug)]
pub struct PlacementSettings {
    pub connect_timeout: Duration,
    pub reconnect_delay: Duration,
    pub stop_grace: Duration,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
