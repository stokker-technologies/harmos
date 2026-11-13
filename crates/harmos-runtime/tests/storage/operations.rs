/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "generated_follow_budget.rs"]
mod follow_budget;
#[path = "generated_foreign_cursor.rs"]
mod foreign_cursor;
#[path = "generated_record_follow.rs"]
mod record_follow;
#[path = "generated_stream_follow.rs"]
mod stream_follow;

#[path = "operation_deletion.rs"]
mod deletion;
#[path = "operation_files.rs"]
mod files;
#[path = "operation_objects.rs"]
mod objects;
#[cfg(feature = "plugin-guest")]
#[path = "operation_plugins.rs"]
mod plugins;
#[path = "operation_queries.rs"]
mod queries;
#[path = "operation_schemas.rs"]
mod schemas;
#[path = "operation_writes.rs"]
mod writes;

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports

// Local Imports
use super::{Fixture, config, write};
use harmos_operations::storage::storage_catalog;

/* === Private Functions === */

fn followed(item: harmos_protocol::storage::Followed) -> harmos_runtime::storage::StoredRecord {
    match item.followed {
        Some(harmos_protocol::storage::followed::Followed::Record(record)) => record.try_into().unwrap(),
        _ => panic!("boundaries are reported once, before records"),
    }
}

fn records() -> harmos_runtime::storage::query::RecordSelection {
    harmos_runtime::storage::query::RecordSelection {
        schema: super::schema(),
        scope: None,
        item: None,
        source_start: None,
        source_end: None,
    }
}

/* === Tests === */

#[test]
fn catalog_serves_operations() {
    let contracts: Vec<_> = storage_catalog().operations.iter().map(|factory| factory.contract().unwrap()).collect();
    let served: Vec<_> =
        contracts.iter().filter(|contract| contract.grpc().is_some()).map(|contract| contract.operation().id.to_string()).collect();
    for required in [
        "harmos.v1.Storage/ListRows",
        "storage.read_file",
        "harmos.v1.Storage/AppendRecords",
        "harmos.v1.Storage/AppendSamples",
        "harmos.v1.Storage/AppendObjectRevisions",
        "storage.write_file",
    ] {
        assert!(served.iter().any(|id| id == required), "missing {required}");
    }
    for retired in ["storage_read_records", "storage_read_objects", "storage_query_stream", "storage_read_files", "storage_reductions"] {
        assert!(!served.iter().any(|id| id == retired), "retired {retired}");
    }
}
