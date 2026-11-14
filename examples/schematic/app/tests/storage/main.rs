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

use arrow_array::UInt64Array;
use harmos::encoding::Message as WireMessage;
use harmos_protocol::client::{
    InvocationsInvokeRequest, OperationsGetRequest, OperationsListRequest, invocations_client::InvocationsClient,
    operations_client::OperationsClient,
};
use harmos_protocol::v1::{FileMetadata, PageRequest, UploadState};
use harmos_query::columnar::ColumnKind;
use harmos_runtime::Identifier;
use harmos_runtime::builtins::storage as operations;
use harmos_runtime::builtins::storage::{
    ColumnSelection, ColumnSource, CreateSnapshotRequest, DeleteSnapshotRequest, GetObjectRequest, ListColumnRowsRequest, MutationId,
    SchemaInput, SetObjectRequest, set_object_response::Outcome,
};
use harmos_runtime::clock::Clock;
use harmos_runtime::operations::NativeHost;
use harmos_runtime::{
    Identified, Message,
    storage::{
        ObjectCatalog, ObjectDefinition, ObjectRetention, StoredObject,
        config::{StorageFilesystemSettings, StorageRequestSettings, StorageResponseSettings, StorageSettings},
    },
};
use harmos_storage::ReadLimits;
use schematic_app::operations::transfer::{
    FilesHash, FilesRead, UploadsAppend, UploadsCreate, UploadsDelete, UploadsGet, UploadsPublish, UploadsResume,
};
use schematic_app::protocol::{
    FilesHashRequest, FilesReadRequest, FilesReadResponse, UploadMutation, UploadsAppendRequest, UploadsCreateRequest,
    UploadsDeleteRequest, UploadsGetRequest, UploadsPublishRequest, UploadsResumeRequest, files_hash_request::Source,
};
use schematic_app::{
    app::Runtime,
    config::{LaunchConfig, Settings},
    document::Document,
    services,
};
use sha2::{Digest, Sha256};
use std::{
    num::NonZeroUsize,
    time::{Duration, SystemTime},
};

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
    let settings = StorageSettings {
        transfer: Default::default(),
        policy: Default::default(),
        request: StorageRequestSettings {
            max_file_productions: NonZeroUsize::new(256).unwrap(),
            max_mutation_receipts: NonZeroUsize::new(1024).unwrap(),
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
    let mut manager = None;
    let document = Document::new();
    let launch = LaunchConfig {
        document: document.clone(),
        settings: Settings::new(directory.path().join("recordings")),
        plugins: false,
    };
    let (clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let runtime = Runtime::launch_with(document.into_origin(), Some(launch), |setup| {
        setup.settings().clock(clock);
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(16 * 1024 * 1024).unwrap()),
            ..Default::default()
        });
        manager = Some(services::queries::attach(setup, catalog.clone(), settings).unwrap());
    })
    .await
    .unwrap();
    let storage = manager.unwrap();

    // Retain a Saved Preview Through the Private Native Operations.
    let first = storage.write_object(&Preview { id: "board".into(), symbols: 2 }).await.unwrap();
    storage.flush().await.unwrap();
    let retained = runtime.operations.call_native::<operations::CreateSnapshot>(CreateSnapshotRequest { at: None }).await.unwrap();
    let saved_cut = retained.resolved_at.unwrap();
    let saved = retained.snapshot.unwrap();
    assert_eq!(saved.at.as_ref().unwrap().sequence, first.position.sequence);
    let schema = harmos_runtime::schema::graph::Schema::from_declaration(
        <Preview as harmos_runtime::schema::Described>::DEFINITION,
        &Default::default(),
    )
    .unwrap();
    let encoded = catalog.encode(&Preview { id: "board".into(), symbols: 3 }, NonZeroUsize::new(4096).unwrap()).unwrap();
    let source = ColumnSource {
        kind: ColumnKind::Objects,
        schema: encoded.schema.clone(),
    };
    let mutation = storage.mutation_window().unwrap();
    let current = runtime
        .operations
        .call_native::<operations::GetObject>(GetObjectRequest {
            schema: Some(encoded.schema.clone()),
            item: "board".into(),
            at: None,
        })
        .await
        .unwrap();
    let request = SetObjectRequest {
        mutation: Some(MutationId {
            window: mutation.id,
            key: Identifier::new("preview-update").unwrap(),
        }),
        expected_revision: current.revision,
        create_if_absent: false,
        schema: Some(SchemaInput {
            key: Some(encoded.schema.clone()),
            graph: Some(schema),
        }),
        value: Some(encoded),
    };

    // Replace the Live Preview and Refuse a Stale Writer.
    assert!(matches!(
        runtime.operations.call_native::<operations::SetObject>(request.clone()).await.unwrap().outcome,
        Some(Outcome::Applied(_))
    ));
    let mut stale = request;
    stale.mutation.as_mut().unwrap().key = Identifier::new("stale-update").unwrap();
    assert!(matches!(runtime.operations.call_native::<operations::SetObject>(stale).await.unwrap().outcome, Some(Outcome::Conflict(_))));

    // Read the Saved Value and Release Its Cut.
    let limits = ReadLimits {
        rows: NonZeroUsize::new(8).unwrap(),
        bytes: NonZeroUsize::new(65536).unwrap(),
    };
    let page = runtime
        .operations
        .call_native::<operations::ListColumnRows>(ListColumnRowsRequest {
            at: Some(saved_cut),
            page: Some(PageRequest {
                page_size: 8,
                max_bytes: 65536,
                page_token: String::new(),
            }),
            selection: Some(ColumnSelection {
                source: Some(source),
                columns: vec!["payload.symbols".into()],
                work: Some(limits),
                predicate: None,
                time: None,
                start: 0,
            }),
        })
        .await
        .unwrap()
        .page
        .unwrap();
    let value = page.batch.column(0).as_any().downcast_ref::<UInt64Array>().unwrap();
    assert_eq!(value.values().as_ref(), &[2]);
    runtime.operations.call_native::<operations::DeleteSnapshot>(DeleteSnapshotRequest { snapshot: Some(saved) }).await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn application_upload_fences_writers_and_publishes_readable_content() {
    // Compose the Query Backend Beside the Document Adapter.
    let directory = tempfile::tempdir().unwrap();
    let catalog = ObjectCatalog::new();
    let settings = StorageSettings {
        transfer: Default::default(),
        policy: Default::default(),
        request: StorageRequestSettings {
            max_file_productions: NonZeroUsize::new(256).unwrap(),
            max_mutation_receipts: NonZeroUsize::new(1024).unwrap(),
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
    let mut manager = None;
    let document = Document::new();
    let launch = LaunchConfig {
        document: document.clone(),
        settings: Settings::new(directory.path().join("recordings")),
        plugins: false,
    };
    let (clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let runtime = Runtime::launch_with(document.into_origin(), Some(launch), |setup| {
        setup.settings().clock(clock);
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(16 * 1024 * 1024).unwrap()),
            ..Default::default()
        });
        manager = Some(services::queries::attach(setup, catalog.clone(), settings).unwrap());
    })
    .await
    .unwrap();
    let _storage = manager.unwrap();

    // Read the Storage-Owned Replay Window Through the Application Resource.
    let info = runtime.operations.call_protobuf::<UploadsGet>(UploadsGetRequest { upload: None }).await.unwrap();
    let window = info.mutations.unwrap().id;
    let payload = b"schematic file";
    let sha256 = Sha256::digest(payload).to_vec();

    // Create an Upload and Append Its First Accepted Prefix.
    let created = runtime
        .operations
        .call_protobuf::<UploadsCreate>(UploadsCreateRequest {
            metadata: Some(FileMetadata {
                id: "schematic-file".into(),
                scope: None,
                media_type: "application/octet-stream".into(),
                expected_length: Some(payload.len() as u64),
                expected_sha256: Some(sha256.clone()),
            }),
            mutation: Some(UploadMutation {
                window: window.clone(),
                key: "create-file".into(),
            }),
        })
        .await
        .unwrap();
    let original = created.upload.unwrap();
    let appended = runtime
        .operations
        .call_protobuf::<UploadsAppend>(UploadsAppendRequest {
            upload: Some(original.clone()),
            offset: 0,
            bytes: payload.to_vec(),
            mutation: Some(UploadMutation {
                window: window.clone(),
                key: "append-file".into(),
            }),
        })
        .await
        .unwrap();
    assert_eq!(appended.accepted_length, payload.len() as u64);

    // Read Current Progress and Fence the Original Writer.
    let current = runtime.operations.call_protobuf::<UploadsGet>(UploadsGetRequest { upload: Some(original.clone()) }).await.unwrap();
    let mut writer = current.upload.unwrap();
    assert_eq!(writer.accepted_length, payload.len() as u64);
    let resumed = runtime
        .operations
        .call_protobuf::<UploadsResume>(UploadsResumeRequest {
            upload: Some(writer.clone()),
            mutation: Some(UploadMutation {
                window: window.clone(),
                key: "resume-file".into(),
            }),
        })
        .await
        .unwrap();
    assert_ne!(resumed.writer_generation, writer.writer_generation);
    writer.writer_generation = resumed.writer_generation;
    let stale = runtime
        .operations
        .call_protobuf::<UploadsAppend>(UploadsAppendRequest {
            upload: Some(original),
            offset: payload.len() as u64,
            bytes: b"stale".to_vec(),
            mutation: Some(UploadMutation {
                window: window.clone(),
                key: "stale-append".into(),
            }),
        })
        .await;
    assert!(stale.is_err());

    // Publish Immutable Content and Keep Its Durable Receipt.
    let published = runtime
        .operations
        .call_protobuf::<UploadsPublish>(UploadsPublishRequest {
            upload: Some(writer),
            length: payload.len() as u64,
            sha256: sha256.clone(),
            mutation: Some(UploadMutation {
                window: window.clone(),
                key: "publish-file".into(),
            }),
        })
        .await
        .unwrap();
    assert!(published.receipt.unwrap().durable_at.is_some());
    let file = published.file.unwrap();

    // Discover the Application Contract and Use Its Generic Call Path.
    runtime
        .operations
        .register_batch(
            harmos_runtime::operations::Registration::new(Default::default())
                .protobuf::<harmos_runtime::builtins::host::List>()
                .unwrap()
                .protobuf::<harmos_runtime::builtins::host::Get>()
                .unwrap(),
        )
        .await
        .unwrap();
    runtime.operations.register_catalog(harmos_runtime::builtins::lifecycle_catalog(), Default::default()).await.unwrap();
    let listener = runtime
        .grpc()
        .attach(harmos_runtime::grpc::Settings {
            address: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap();
    let address = format!("http://{}", listener.local_addr());
    let mut registry = OperationsClient::connect(address.clone()).await.unwrap();
    let listed = registry.list(OperationsListRequest {}).await.unwrap().into_inner();
    let discovered = listed
        .operations
        .iter()
        .find(|entry| entry.contract.as_ref().is_some_and(|contract| contract.id == "schematic.v1.Files/Read"))
        .unwrap();
    let selected = registry.get(OperationsGetRequest { id: "schematic.v1.Files/Read".into() }).await.unwrap().into_inner();
    assert_eq!(selected.operation.as_ref(), Some(discovered));
    let contract = discovered.contract.as_ref().unwrap();
    let protobuf = contract.protobuf.as_ref().unwrap();
    assert_eq!(protobuf.request, "schematic.v1.FilesReadRequest");
    assert_eq!(protobuf.response, "schematic.v1.FilesReadResponse");
    assert!(!protobuf.descriptors.is_empty());
    assert!(contract.policy.is_some());
    assert_eq!(contract.grpc.as_ref().unwrap().method, "Read");
    let request = FilesReadRequest {
        file: Some(file.clone()),
        offset: 0,
        max_bytes: 4,
        at: None,
    };
    let mut invocations = InvocationsClient::connect(address).await.unwrap();
    let response = invocations
        .invoke(InvocationsInvokeRequest {
            operation: contract.id.clone(),
            fingerprint: contract.fingerprint.clone(),
            payload: request.encode_to_vec(),
        })
        .await
        .unwrap()
        .into_inner();
    let generic = <FilesReadResponse as WireMessage>::decode(response.payload.as_slice()).unwrap();
    assert_eq!(generic.bytes, payload[..4]);
    drop(invocations);
    drop(registry);
    drop(listener);

    // Read Bounded Ranges and Hash the Published File Through Application Calls.
    let prefix = runtime
        .operations
        .call_protobuf::<FilesRead>(FilesReadRequest {
            file: Some(file.clone()),
            offset: 0,
            max_bytes: 4,
            at: None,
        })
        .await
        .unwrap();
    assert_eq!(prefix.bytes, &payload[..4]);
    assert!(!prefix.complete);
    let suffix = runtime
        .operations
        .call_protobuf::<FilesRead>(FilesReadRequest {
            file: Some(file.clone()),
            offset: 4,
            max_bytes: payload.len() as u64,
            at: None,
        })
        .await
        .unwrap();
    assert_eq!(suffix.bytes, &payload[4..]);
    assert!(suffix.complete);
    let hashed = runtime
        .operations
        .call_protobuf::<FilesHash>(FilesHashRequest {
            source: Some(Source::File(file)),
            offset: 0,
            length: None,
            at: None,
        })
        .await
        .unwrap();
    assert_eq!(hashed.read_bytes, payload.len() as u64);
    assert_eq!(hashed.sha256, sha256);

    // Abandon an Unfinished Upload Without Publishing a File.
    let unfinished = runtime
        .operations
        .call_protobuf::<UploadsCreate>(UploadsCreateRequest {
            metadata: Some(FileMetadata {
                id: "unfinished-file".into(),
                ..Default::default()
            }),
            mutation: Some(UploadMutation {
                window: window.clone(),
                key: "create-unfinished".into(),
            }),
        })
        .await
        .unwrap()
        .upload
        .unwrap();
    let deleted = runtime
        .operations
        .call_protobuf::<UploadsDelete>(UploadsDeleteRequest {
            upload: Some(unfinished),
            mutation: Some(UploadMutation { window, key: "abandon-file".into() }),
        })
        .await
        .unwrap();
    assert_eq!(deleted.state, UploadState::Abandoned as i32);
    runtime.shutdown().await.unwrap();
}
