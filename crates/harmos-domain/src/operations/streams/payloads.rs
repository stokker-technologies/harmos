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

// One Ordered Batch On One Stream: The Head Row Carries `sequence`; Each
// Following Row Takes The Next Sequence. A Single Row Has No Following Rows.
payload! { PublishRequest,
    stream: String => (1, Kind::String, Required),
    schema: u32 => (2, Kind::U32, Required),
    sequence: Option<u64> => (3, Kind::U64, Optional),
    published_unix_micros: Option<i64> => (4, Kind::I64, Optional),
    context: Option<String> => (5, Kind::String, Optional),
    payload: Vec<u8> => (6, Kind::Bytes, Required),
    following: Vec<FollowingRow> => (7, Kind::Message { definition: FollowingRow::DEFINITION }, Repeated),
}

// A Following Row's Stamp Is Its Signed Offset From The Preceding Row's
// Stamp, So A Steady Series Costs A Short Varint Per Row. It Is Present
// Exactly When The Row Is Stamped, And Requires A Stamped Predecessor.
payload! { FollowingRow,
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
