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
use crate::operations::common::payload::payload;

/* === Constants === */

/* === Macros === */
payload! { Status,
    instance: u64 => (1, Kind::U64, Required),
}

payload! { Cancel,
    instance: u64 => (1, Kind::U64, Required),
}

payload! { Watch,
    instance: u64 => (1, Kind::U64, Required),
}

payload! { InstanceState,
    phase: String => (1, Kind::String, Required),
    outcome: String => (2, Kind::String, Required),
    cancellation_requested: bool => (3, Kind::Bool, Required),
    attempt: u32 => (4, Kind::U32, Required),
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
