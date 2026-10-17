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
mod execute;
mod object_schemas;
mod reader;
mod source;
pub mod transform;
mod typed;

/* === Re-Exports === */
pub use accumulate::{Accumulator, Scratch};
pub use harmos_domain::model::storage::query::{
    FileSelection, ObjectSelection, ObjectVersions, QueryCompletion, QueryOutput, RecordSelection,
    RowSlice, SchemaSelection, Source, StreamSelection, scalar,
};
pub(crate) use reader::schema_at;
pub use reader::{QueryArrow, QueryReader};
pub use typed::{ObjectRead, ObjectReader, RecordRead, RecordReader};

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
