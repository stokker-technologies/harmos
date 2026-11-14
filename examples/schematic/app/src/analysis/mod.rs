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
use std::io;
use harmos_runtime::{Message, storage::StorageManager};
use harmos_query::typed::StreamRead;
use harmos_domain::model::storage::query::StreamSelection;

// Third Party Imports

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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub async fn read(storage: &StorageManager) -> io::Result<Columns> {
    let mut reader = storage.list_samples(StreamRead {
        at: storage.get_saved_position(),
        selection: StreamSelection { schema: crate::storage::voltage_schema(), scope: None, item: None, stream: None, source_start: None, source_end: None, after: 0 },
    }).await.map_err(io::Error::other)?;
    let mut columns = Columns { position: Vec::new(), gap_missed: Vec::new() };
    while let Some(page) = reader.next().await.map_err(io::Error::other)? {
        for row in page.iter() {
            let value = crate::storage::SavedVoltage::decode(&row.record.payload).map_err(io::Error::other)?;
            columns.position.push(value.position);
            columns.gap_missed.push(value.gap_missed);
        }
    }
    Ok(columns)
}

/* === Private Functions === */

/* === Tests === */
