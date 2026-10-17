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
payload! { Operations,
    after: Option<String> => (1, Kind::String, Optional),
    rows: Option<u32> => (2, Kind::U32, Optional),
    bytes: Option<u32> => (3, Kind::U32, Optional),
}

// Catalog Is A UTF-8 JSON Array Of Canonical DiscoveredOperation Values.
payload! { OperationsResponse,
    catalog: Vec<u8> => (1, Kind::Bytes, Required),
    next: Option<String> => (2, Kind::String, Optional),
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
