/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Saved Preview Reads Survive Atomic Replacement.

/* === Imports === */

use harmos::storage as wire;
use harmos_protocol::v1;
use harmos_runtime::builtins::storage as operations;
use harmos_runtime::{
    Identified, Message,
    storage::{
        ObjectCatalog, ObjectDefinition, ObjectRetention, StoreHistory, StoredObject,
        config::{StorageFilesystemSettings, StorageRequestSettings, StorageResponseSettings, StorageSettings},
    },
};
use schematic_app::{
    app::Runtime,
    config::{LaunchConfig, Settings},
    document::Document,
    services,
};
use std::{num::NonZeroUsize, time::Duration};

/* === Structs === */

#[derive(Message, Clone, Debug, Default)]
struct Preview {
    id: String,
    symbols: u64,
}

/* === Implementations === */

impl Identified for Preview {
    const ID: &'static str = "schematic.preview";
}
impl StoredObject for Preview {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::All;
    fn key(&self) -> &String {
        &self.id
    }
}

/* === Tests === */

#[tokio::test]
async fn saved_preview_survives_atomic_replacement() {
    // Compose the Query Backend Beside the Document Adapter.
    let directory = tempfile::tempdir().unwrap();
    let catalog = ObjectCatalog::new().entry(ObjectDefinition::of::<Preview>());
    let history = StoreHistory {
        store: harmos_runtime::Identifier::new("schematic-example").unwrap(),
        epoch: harmos_runtime::Identifier::new("first").unwrap(),
    };
    let settings = StorageSettings {
        transfer: Default::default(),
        policy: Default::default(),
        request: StorageRequestSettings {
            max_file_productions: std::num::NonZeroUsize::new(256).unwrap(),
            max_mutation_receipts: std::num::NonZeroUsize::new(1024).unwrap(),
            max_pending_queries: NonZeroUsize::new(4).unwrap(),
            max_active_queries: NonZeroUsize::new(2).unwrap(),
            max_pending_write_batches: NonZeroUsize::new(4).unwrap(),
            max_active_file_operations: NonZeroUsize::new(2).unwrap(),
            max_pending_write_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        },
        response: StorageResponseSettings {
            batch_rows: NonZeroUsize::new(64).unwrap(),
            batch_bytes: NonZeroUsize::new(65536).unwrap(),
        },
        filesystem: StorageFilesystemSettings {
            poll_interval: Duration::from_secs(1),
            max_entries: NonZeroUsize::new(64).unwrap(),
            event_capacity: NonZeroUsize::new(64).unwrap(),
        },
    };
    let recovery = harmos_runtime::storage::state::StorageStateSettings {
        max_entry_bytes: NonZeroUsize::new(65536).unwrap(),
        max_checkpoint_bytes: NonZeroUsize::new(65536).unwrap(),
    };
    let mut manager = None;
    let document = Document::new(directory.path().join("document.json"));
    let launch = LaunchConfig {
        document: document.clone(),
        settings: Settings::new(directory.path().join("recordings")),
    };
    let runtime = Runtime::launch_with(document.into_origin(), Some(launch), |setup| {
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(16 * 1024 * 1024).unwrap()),
            ..Default::default()
        });
        manager = Some(
            services::queries::attach(setup, &directory.path().join("queries"), history, catalog.clone(), settings, recovery).unwrap(),
        );
    })
    .await
    .unwrap();
    let storage = manager.unwrap();

    // Retain a Saved Preview Through the Client-Face Contract.
    let first = storage.write_object(&Preview { id: "board".into(), symbols: 2 }).await.unwrap();
    storage.flush().await.unwrap();
    let retained =
        runtime.operations.call_protobuf::<operations::CreateSnapshot>(v1::StorageCreateSnapshotRequest { at: None }).await.unwrap();
    let saved_cut = retained.resolved_at.unwrap();
    let saved = retained.snapshot.unwrap();
    assert_eq!(saved.at.as_ref().unwrap().sequence, first.position.sequence);
    let schema = harmos_runtime::schema::graph::Schema::from_declaration(
        <Preview as harmos_runtime::schema::Described>::DEFINITION,
        &Default::default(),
    )
    .unwrap();
    let encoded = catalog.encode(&Preview { id: "board".into(), symbols: 3 }, NonZeroUsize::new(4096).unwrap()).unwrap();
    let source = wire::ColumnSource {
        kind: wire::ColumnKind::Objects as i32,
        schema: Some(encoded.schema.clone().into()),
    };
    let mutation = storage.mutation_window().unwrap();
    let current = runtime
        .operations
        .call_protobuf::<operations::GetObject>(v1::StorageGetObjectRequest {
            schema: Some(encoded.schema.clone().into()),
            item: "board".into(),
            at: None,
        })
        .await
        .unwrap();
    let request = v1::StorageSetObjectRequest {
        mutation: Some(wire::MutationId {
            window: mutation.id.to_string(),
            key: "preview-update".into(),
        }),
        expected_revision: current.revision,
        create_if_absent: false,
        schema: Some(wire::SchemaInput {
            key: Some(encoded.schema.clone().into()),
            graph: Some(schema.into()),
        }),
        value: Some(encoded.into()),
    };

    // Replace the Live Preview and Refuse a Stale Writer.
    assert!(matches!(
        runtime.operations.call_protobuf::<operations::SetObject>(request.clone()).await.unwrap().outcome,
        Some(v1::storage_set_object_response::Outcome::Applied(_))
    ));
    let mut stale = request;
    stale.mutation.as_mut().unwrap().key = "stale-update".into();
    assert!(matches!(
        runtime.operations.call_protobuf::<operations::SetObject>(stale).await.unwrap().outcome,
        Some(v1::storage_set_object_response::Outcome::Conflict(_))
    ));

    // Read the Saved Value and Release Its Cut.
    let limits = wire::ReadLimits { rows: 8, bytes: 65536 };
    let page = runtime
        .operations
        .call_protobuf::<operations::ListColumnRows>(v1::StorageListColumnRowsRequest {
            at: Some(saved_cut),
            page: Some(v1::PageRequest {
                page_size: 8,
                max_bytes: 65536,
                page_token: String::new(),
            }),
            selection: Some(wire::ColumnSelection {
                source: Some(source),
                columns: vec!["payload.symbols".into()],
                work: Some(limits),
                ..Default::default()
            }),
        })
        .await
        .unwrap()
        .page
        .unwrap();
    let batch = harmos_storage::query::protobuf::decode_batch(page.ipc, limits.try_into().unwrap()).unwrap();
    let value = batch.as_record_batch().column(0).as_any().downcast_ref::<arrow_array::UInt64Array>().unwrap();
    assert_eq!(value.values().as_ref(), &[2]);
    runtime
        .operations
        .call_protobuf::<operations::DeleteSnapshot>(v1::StorageDeleteSnapshotRequest { snapshot: Some(saved) })
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
}
