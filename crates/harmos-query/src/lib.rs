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

pub mod arrow;
pub mod batches;
mod reader;
pub mod columnar;
pub mod config;
pub mod execute;
pub mod index;
pub mod merge;
pub mod quantiles;
pub mod source;
pub mod table;
pub mod time_buckets;
pub mod transform;
pub mod typed;
mod accumulate;
mod aggregate;
mod error;
mod order_stats;
mod ordering;
mod reduction;
mod spill;
pub mod statistics;

/* === Re-Exports === */

pub use reader::{QueryArrow, QueryOutput, QueryReader, QueryResponse, QueryStep};
pub use error::{BackendFailure, Error, Result};
pub use accumulate::Accumulator;
pub use table::{Table, TableReader, TableWriter};
pub use harmos_domain::{Aggregate, Identifier, MemorySize, Message, btree_map_memory, schema};
pub use harmos_domain::model::storage::{ObjectEntry, PayloadEncoding, ReadBatch, SchemaKey, StoreHistory, StoredRecord, StoredRow};
pub use harmos_domain::model::storage::query::{RowSlice, scalar};
#[cfg(feature = "testing")]
pub use reduction::testing::Coordinates;
#[cfg(feature = "testing")]
pub use source::testing::PayloadDecoder;


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
