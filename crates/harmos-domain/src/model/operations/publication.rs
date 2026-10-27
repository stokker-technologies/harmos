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

/* === Constants === */

/* === Macros === */

crate::builtin_payload! { FollowingRow,
    payload: Vec<u8> => (1, Kind::Bytes, Required),
    published_delta_micros: Option<i64> => (2, Kind::I64, Optional),
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
