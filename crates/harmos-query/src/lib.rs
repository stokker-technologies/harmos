/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Arrow Query Execution Without Storage Ownership

/* === Modules === */

mod accumulate;
mod aggregate;
pub mod arrow;
pub mod batches;
pub mod columnar;
pub mod config;
mod error;
pub mod execute;
pub mod index;
pub mod merge;
mod order_stats;
mod ordering;
pub mod quantiles;
mod reader;
mod reduction;
pub mod source;
mod spill;
pub mod statistics;
pub mod table;
pub mod time_buckets;
pub mod transform;
pub mod typed;

/* === Re-Exports === */

pub use accumulate::Accumulator;
pub use error::{BackendFailure, Error, Result};
pub use harmos_domain::model::storage::query::{RowSlice, scalar};
pub use harmos_domain::model::storage::{ObjectEntry, PayloadEncoding, ReadBatch, SchemaKey, StoreHistory, StoredRecord, StoredRow};
pub use harmos_domain::{Aggregate, Identifier, MemorySize, Message, btree_map_memory, schema};
pub use reader::{QueryArrow, QueryOutput, QueryReader, QueryResponse, QueryStep};
#[cfg(feature = "testing")]
pub use reduction::testing::Coordinates;
#[cfg(feature = "testing")]
pub use source::testing::PayloadDecoder;
pub use table::{Table, TableReader, TableWriter};

/* === Imports === */

#[cfg(test)]
extern crate harmos_domain as harmos;

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
