/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Bounded Batch Sources At An Immutable Cut

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::pin::Pin;

// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;

// Local Imports
use harmos_domain::model::storage::{ReadLimits, Source, StorageCursor};
use harmos_domain::model::storage::query::transform::{filter::Filter, project::Project};
use crate::Result;


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type QueryFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/* === Structs === */

pub struct BatchScan {
    pub limits: ReadLimits,
    pub projection: Option<Project>,
    pub pruning: Option<Filter>,
}

pub struct BatchPage {
    pub batch: RecordBatch,
    pub complete: bool,
}


/* === Enums === */

/* === Traits === */

pub trait BatchSource: Send + Sync {
    fn open<'a>(
        &'a self, selection: &'a Source, at: &'a StorageCursor, scan: &'a BatchScan,
    ) -> QueryFuture<'a, Box<dyn BatchReader>>;
}

pub trait BatchReader: Send {
    fn schema(&self) -> SchemaRef;
    fn at(&self) -> &StorageCursor;
    fn next(&mut self) -> QueryFuture<'_, BatchPage>;
}


/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
