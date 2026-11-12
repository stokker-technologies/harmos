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

mod accumulate;
pub mod columnar;
mod execute;
mod object_schemas;
mod order_stats;
mod ordering;
pub mod quantiles;
mod reader;
mod reduction;
mod source;
mod spill;
mod table;
pub mod transform;
mod typed;

/* === Re-Exports === */

#[cfg(feature = "testing")]
pub use reduction::testing::Coordinates;
#[cfg(feature = "testing")]
pub use source::testing::PayloadDecoder;

pub use accumulate::Accumulator;
pub use harmos_domain::model::storage::query::{
    FileSelection, ObjectSelection, ObjectVersions, QueryCompletion, RecordSelection, RowSlice, SchemaSelection, Source, StreamSelection,
    scalar,
};
pub use harmos_protocol::storage::QueryOutput;
pub(crate) use reader::schema_at;
pub use reader::{Query, QueryArrow, QueryReader};
pub use table::{Table, TableReader, TableWriter};
pub use typed::{ObjectRead, ObjectReader, RecordRead, RecordReader, StreamRead, StreamReader};

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
