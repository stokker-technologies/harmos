/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Query Operation Story

/* === Modules === */

#[path = "generated_query_admission.rs"]
mod admission;
#[path = "generated_query_batches.rs"]
mod batches;
#[path = "generated_query_projection.rs"]
mod projection;

/* === Imports === */

// Standard Library Imports
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;

// Third Party Imports
use arrow_array::{StringArray, StructArray, UInt64Array};

// Local Imports
use super::super::{Bank, Data, Ledger, Recovery, catalog, history};
use super::{Fixture, config};
use harmos_operations::storage::Query;
use harmos_runtime::storage::Query as Request;
use harmos_runtime::storage::query::scalar::ScalarValue;
use harmos_runtime::storage::query::transform::{
    Transform,
    filter::{Clause, Compare, Condition, Filter, Term},
    project::{Project, Projection},
    sort::{Sort, SortKey},
};
use harmos_runtime::storage::query::{QueryArrow, RecordSelection, RowSlice, Source};
use harmos_runtime::storage::{
    ObjectKey, ObjectRetention, PayloadEncoding, RecordInput, SchemaKey, StorageBackends, StorageManager, StoredObject,
};
use harmos_runtime::{Identified, Identifier, Message};

/* === Macros === */

macro_rules! object {
    ($ty:ty, $name:literal) => {
        impl Identified for $ty {
            const ID: &'static str = $name;
        }
        impl StoredObject for $ty {
            type Key = String;
            const RETENTION: ObjectRetention = ObjectRetention::UntilReleased;
            fn key(&self) -> &String {
                &self.id
            }
        }
    };
}

object!(First, "test.query.first");

object!(Second, "test.query.second");

object!(Third, "test.query.third");

/* === Structs === */

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct Reading {
    value: u64,
}

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct First {
    id: String,
    value: u64,
}

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct Second {
    id: String,
    value: u64,
    annotation: String,
}

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct Third {
    id: String,
    value: u64,
}

/* === Enums === */

fn objects_catalog() -> harmos_domain::model::storage::ObjectCatalog {
    harmos_domain::model::storage::ObjectCatalog::new()
        .entry(harmos_domain::model::storage::ObjectDefinition::of::<First>())
        .entry(harmos_domain::model::storage::ObjectDefinition::of::<Second>())
        .entry(harmos_domain::model::storage::ObjectDefinition::of::<Third>())
}
