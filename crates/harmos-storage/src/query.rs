/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Declarative Storage Queries over Arrow

/* === Modules === */

pub mod columnar;
pub mod index;
mod manager;
mod batches;
mod object_schemas;
pub mod projection;

mod reader;
mod typed;

/* === Re-Exports === */


pub use harmos_domain::model::storage::query::{
    FileSelection, ObjectSelection, ObjectVersions, QueryCompletion, RecordSelection, RowSlice, SchemaSelection, Source, StreamSelection,
    scalar,
};

pub(crate) use reader::schema_at;
pub use reader::QueryReader;



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
