/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Declarative Storage Queries Over Arrow

mod accumulate;
mod execute;
mod object_schemas;
mod order_stats;
mod ordering;
mod reader;
mod reduction;
mod source;
mod spill;
mod table;
pub mod transform;
mod typed;

pub use accumulate::Accumulator;
pub use harmos_domain::model::storage::query::{
    FileSelection, ObjectSelection, ObjectVersions, QueryCompletion, QueryOutput, RecordSelection, RowSlice, SchemaSelection, Source,
    StreamSelection, scalar,
};
pub(crate) use reader::schema_at;
pub use reader::{Query, QueryArrow, QueryReader};
pub use table::{Table, TableReader, TableWriter};
pub use typed::{ObjectRead, ObjectReader, RecordRead, RecordReader, StreamRead, StreamReader};
