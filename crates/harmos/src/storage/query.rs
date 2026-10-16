/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Declarative Storage Queries Over Arrow

/* === Modules === */
mod accumulate;
mod contract;
mod execute;
mod object_schemas;
mod reader;
pub mod scalar;
mod source;
pub mod transform;
mod typed;

/* === Re-Exports === */
pub use accumulate::{Accumulator, Scratch};
pub use contract::{
    FileSelection, ObjectSelection, ObjectVersions, Query, QueryCompletion, QueryOutput,
    RecordSelection, RowSlice, SchemaSelection, Source, StreamSelection,
};
pub use reader::QueryReader;
pub(in crate::storage) use reader::schema_at;
pub use typed::{ObjectRead, ObjectReader, RecordRead, RecordReader};

/* === Imports === */

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
