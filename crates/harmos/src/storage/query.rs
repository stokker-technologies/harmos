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
pub(crate) mod accumulate;
mod contract;
mod execute;
mod object_schemas;
mod reader;
pub mod scalar;
mod source;
pub mod transform;

/* === Re-Exports === */
pub use contract::{
    FileSelection, ObjectSelection, ObjectVersions, Query, QueryCompletion, QueryOutput,
    RecordSelection, RowSlice, SchemaSelection, Source, StreamSelection,
};
pub use reader::QueryReader;
pub(in crate::storage) use reader::schema_at;

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
