/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Consistent Offline Copy

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::storage::{Backend, Fixture, config, history, record, schema};
use harmos_operations::storage::{self, CopyBackend, CopyBackendRequest, DisposeBackend, DisposeBackendRequest};
use harmos_query::columnar::{Cancellation, ColumnKind, ColumnSource};
use harmos_runtime::config::SchemaSettings;
use harmos_runtime::schema::{Described, graph::Schema};
use harmos_runtime::storage::{ReadLimits, StorageBackend};
use harmos_runtime::{Identifier, Message};
use harmos_storage::query::projection::{ProjectionDefinition, ProjectionTransform};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Message, Clone, Debug, Default)]
struct Sample {
    value: f64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn maintenance_copy_is_consistent_at_one_cut() {
    // Keep an Earlier Cut While New Writes Advance the Source
    let fixture = Fixture::new(config()).await;
    fixture.runtime.operations.register_catalog(storage::storage_catalog(), Default::default()).await.unwrap();
    let graph = Schema::from_declaration(Sample::DEFINITION, &SchemaSettings::default()).unwrap();
    fixture.storage.write_schema(schema(), graph).await.unwrap();
    let payload = Sample { value: 1.0 }.encode();
    let first = fixture.storage.append_records(vec![record(&schema(), &Identifier::new("first").unwrap(), &payload)]).await.unwrap();
    fixture.storage.flush().await.unwrap();
    let retained = fixture.storage.snapshot(&first).await.unwrap();
    let payload = Sample { value: 2.0 }.encode();
    fixture.storage.append_records(vec![record(&schema(), &Identifier::new("later").unwrap(), &payload)]).await.unwrap();
    fixture.storage.flush().await.unwrap();
    // A Later Projection Output Cannot Enter the Earlier Copy
    let definition = ProjectionDefinition {
        name: Identifier::new("values").unwrap(),
        version: 1,
        source: ColumnSource {
            kind: ColumnKind::Records,
            schema: schema(),
        },
        transform: ProjectionTransform::Select(vec!["payload.value".into()]),
    };
    fixture.data.set_projection_definition(&definition, None, false, None).await.unwrap();
    let source = fixture.storage.snapshot(&fixture.storage.get_saved_position()).await.unwrap();
    let limits = ReadLimits {
        rows: NonZeroUsize::new(32).unwrap(),
        bytes: NonZeroUsize::new(1048576).unwrap(),
    };
    let revision = source.advance_projection(&definition.name, None, limits, &Cancellation::default()).await.unwrap();
    assert!(fixture.data.get_projection_status(&definition.name, revision, None).await.unwrap().is_some());
    let target = Arc::new(Backend::new(history()));
    let request = CopyBackendRequest {
        at: first.clone(),
        target: target.clone(),
    };
    fixture.runtime.operations.call_native::<CopyBackend>(request).await.unwrap();
    assert_eq!(target.get_saved_position().await.unwrap(), first);
    assert!(target.get_projection_status(&definition.name, revision, None).await.unwrap().is_none());
    let request = CopyBackendRequest {
        at: first.clone(),
        target: target.clone(),
    };
    assert!(fixture.runtime.operations.call_native::<CopyBackend>(request).await.is_err());
    assert_eq!(target.records.lock().unwrap().values().map(|record| record.item.as_str().to_owned()).collect::<Vec<_>>(), ["first"]);

    // Refuse Live Disposal and Preserve a Retained Target Snapshot
    target.create_session().await.unwrap();
    assert!(fixture.runtime.operations.call_native::<DisposeBackend>(DisposeBackendRequest { target: target.clone() }).await.is_err());
    let snapshot = target.create_snapshot(&first).await.unwrap();
    target.delete_session().await.unwrap();
    assert!(target.dispose().await.is_err());
    target.delete_snapshot(&snapshot).await.unwrap();
    fixture.runtime.operations.call_native::<DisposeBackend>(DisposeBackendRequest { target: target.clone() }).await.unwrap();
    assert!(target.records.lock().unwrap().is_empty());
    assert_eq!(target.get_saved_position().await.unwrap().sequence, 0);
    drop(source);
    drop(retained);
    fixture.finish().await;
}
