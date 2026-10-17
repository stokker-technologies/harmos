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
use std::path::Path;

// Third Party Imports
use serde::Deserialize;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Debug, PartialEq, Eq)]
pub struct Columns {
    pub position: Vec<u64>,
    pub gap_missed: Vec<Option<u64>>,
}

#[derive(Deserialize)]
struct Witnessed {
    position: u64,
    gap_missed: Option<u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn read(directory: &Path) -> Columns {
    let rows: Vec<Witnessed> =
        schematic_storage::streams::read_rows(directory.join("net_voltage.jsonl"))
            .expect("the clean stop wrote the selected stream");
    let mut columns = Columns {
        position: Vec::with_capacity(rows.len()),
        gap_missed: Vec::with_capacity(rows.len()),
    };

    for row in rows {
        columns.position.push(row.position);
        columns.gap_missed.push(row.gap_missed);
    }

    columns
}

/* === Private Functions === */

/* === Tests === */
