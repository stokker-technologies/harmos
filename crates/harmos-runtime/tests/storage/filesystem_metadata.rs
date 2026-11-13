/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Repeat Observed Metadata And Retain Directory Pages Through Independent Leases

use harmos_operations::storage::{self as operations, context};
use harmos_protocol::{context as child, storage as wire, v1};
use prost::Message;
use sha2::{Digest, Sha256};
use std::{
    num::NonZeroUsize,
    sync::{Arc, atomic::Ordering},
};

#[tokio::test]
async fn filesystem_metadata_cuts_repeat_results_and_pin_directory_continuations() {
    let mut settings = super::config();
    settings.response.batch_rows = NonZeroUsize::new(4).unwrap();
    settings.response.batch_bytes = NonZeroUsize::new(1024).unwrap();
    let ceiling = settings.response.batch_bytes.get();
    let fixture = super::Fixture::new(settings).await;
    crate::support::app::register(&fixture.runtime, operations::storage_catalog(), Default::default()).await.unwrap();
    fixture.runtime.operations.register_authorizer(Arc::new(super::transfer_plugin::AllowTransfers)).unwrap();
    let plugin = super::transfer_plugin::TransferPlugin::start().await;
    let attached = fixture
        .runtime
        .attach_grpc(
            harmos_runtime::plugin::grpc::Placement::Remote(plugin.address.clone()),
            harmos_runtime::plugin::grpc::PlacementSettings {
                connect_timeout: std::time::Duration::from_secs(30),
                reconnect_delay: std::time::Duration::from_secs(1),
                stop_grace: std::time::Duration::from_secs(30),
                stream_window: NonZeroUsize::new(64).unwrap(),
            },
        )
        .await
        .unwrap();
    let listener = fixture.runtime.grpc().attach(Default::default()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("source.bin");
    let original = b"initial-content";
    std::fs::write(&path, original).unwrap();
    let source = wire::FileSource {
        source: Some(wire::file_source::Source::Local(path.to_str().unwrap().into())),
    };
    let hash_request = v1::StorageGetFileHashRequest {
        source: Some(source.clone()),
        range: Some(wire::ByteRange { offset: 1, limit: Some(5) }),
        at: None,
    };
    let first = super::transfer_plugin::client_call::<operations::GetFileHash>(&channel, hash_request.clone()).await.unwrap();
    assert_eq!(first.hash.as_ref().unwrap().bytes_read, 5);
    assert_eq!(first.hash.as_ref().unwrap().sha256, Sha256::digest(&original[1..6]).to_vec());
    assert!(first.encoded_len() <= ceiling);
    std::fs::write(&path, b"changed-content").unwrap();
    let repeated = super::transfer_plugin::call::<context::GetFileHash>(
        &fixture.runtime.operations,
        child::StorageGetFileHashRequest {
            source: Some(source.clone()),
            range: hash_request.range,
            at: first.resolved_at.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(repeated.hash, first.hash);
    assert_eq!(repeated.resolved_at, first.resolved_at);
    assert!(
        super::transfer_plugin::client_call::<operations::GetFileHash>(
            &channel,
            v1::StorageGetFileHashRequest {
                range: Some(wire::ByteRange { offset: 2, limit: Some(5) }),
                at: first.resolved_at.clone(),
                ..hash_request.clone()
            }
        )
        .await
        .is_err()
    );

    let capture = super::transfer_plugin::call::<context::CaptureFile>(
        &fixture.runtime.operations,
        child::StorageCaptureFileRequest {
            source: Some(source.clone()),
            range: Some(wire::ByteRange { offset: 0, limit: None }),
            metadata: Some(wire::FileMetadata {
                id: "unary-file".into(),
                media_type: "application/octet-stream".into(),
                ..Default::default()
            }),
            consistency: wire::CaptureConsistency::Stable as i32,
        },
    )
    .await
    .unwrap();
    let receipt = capture.receipt.unwrap();
    assert!(receipt.completed);
    let captured = capture.file.unwrap();
    assert_eq!(receipt.accepted_at, receipt.durable_at);
    let stored = wire::FileSource {
        source: Some(wire::file_source::Source::Stored(captured.clone())),
    };
    let stored_request = v1::StorageGetFileHashRequest {
        source: Some(stored),
        range: Some(wire::ByteRange { offset: 0, limit: None }),
        at: Some(v1::ReadCut {
            positions: vec![receipt.durable_at.unwrap()],
            revisions: vec![],
        }),
    };
    let stored_hash = super::transfer_plugin::client_call::<operations::GetFileHash>(&channel, stored_request.clone()).await.unwrap();
    assert_eq!(stored_hash.hash.as_ref().unwrap().sha256, captured.sha256);
    fixture.data.control.corrupt_file.store(true, Ordering::Release);
    assert!(super::transfer_plugin::client_call::<operations::GetFileHash>(&channel, stored_request).await.is_err());
    fixture.data.control.corrupt_file.store(false, Ordering::Release);

    let directory = root.path().join("listing");
    std::fs::create_dir(&directory).unwrap();
    for (name, bytes) in [("b.bin", b"bb".as_slice()), ("c.bin", b"ccc".as_slice()), ("a.bin", b"a".as_slice())] {
        std::fs::write(directory.join(name), bytes).unwrap();
    }
    let selection = wire::DirectorySelection {
        root: directory.to_str().unwrap().into(),
        recursive: false,
        rules: vec![wire::DirectoryRule { pattern: "**".into(), include: true }],
    };
    let first_page = super::transfer_plugin::client_call::<operations::ListDirectoryEntries>(
        &channel,
        v1::StorageListDirectoryEntriesRequest {
            selection: Some(selection.clone()),
            at: None,
            page: Some(v1::PageRequest {
                page_size: 1,
                max_bytes: ceiling as u64,
                page_token: String::new(),
            }),
        },
    )
    .await
    .unwrap();
    assert!(first_page.entries[0].path.ends_with("a.bin"));
    assert_eq!(first_page.entries.len(), 1);
    assert!(!first_page.next_page_token.is_empty());
    assert!(first_page.encoded_len() <= ceiling);
    let pinned = fixture
        .runtime
        .operations
        .call_protobuf::<operations::CreateSnapshot>(v1::StorageCreateSnapshotRequest { at: first_page.resolved_at.clone() })
        .await
        .unwrap();
    std::fs::remove_file(directory.join("a.bin")).unwrap();
    std::fs::write(directory.join("aa.bin"), b"new").unwrap();
    std::fs::write(directory.join("b.bin"), b"longer-content").unwrap();
    let second_page = super::transfer_plugin::call::<context::ListDirectoryEntries>(
        &fixture.runtime.operations,
        child::StorageListDirectoryEntriesRequest {
            selection: Some(selection.clone()),
            at: first_page.resolved_at.clone(),
            page: Some(v1::PageRequest {
                page_size: 1,
                max_bytes: ceiling as u64,
                page_token: first_page.next_page_token.clone(),
            }),
        },
    )
    .await
    .unwrap();
    assert!(second_page.entries[0].path.ends_with("b.bin"));
    assert_eq!(second_page.entries[0].length, 2);
    assert_eq!(second_page.resolved_at, first_page.resolved_at);
    for _ in 0..12 {
        super::transfer_plugin::client_call::<operations::GetFileHash>(&channel, hash_request.clone()).await.unwrap();
    }
    // The Original Automatic Entry May Retire; The Explicit Lease Keeps Its Image Identity.
    let third = super::transfer_plugin::client_call::<operations::ListDirectoryEntries>(
        &channel,
        v1::StorageListDirectoryEntriesRequest {
            selection: Some(selection.clone()),
            at: first_page.resolved_at.clone(),
            page: Some(v1::PageRequest {
                page_size: 1,
                max_bytes: ceiling as u64,
                page_token: second_page.next_page_token.clone(),
            }),
        },
    )
    .await
    .unwrap();
    assert!(third.entries[0].path.ends_with("c.bin"));
    assert!(third.next_page_token.is_empty());
    let replay = super::transfer_plugin::client_call::<operations::ListDirectoryEntries>(
        &channel,
        v1::StorageListDirectoryEntriesRequest {
            selection: Some(selection.clone()),
            at: first_page.resolved_at.clone(),
            page: None,
        },
    )
    .await
    .unwrap();
    assert!(replay.entries[0].path.ends_with("a.bin"));
    let mut changed = selection.clone();
    changed.recursive = true;
    assert!(
        super::transfer_plugin::client_call::<operations::ListDirectoryEntries>(
            &channel,
            v1::StorageListDirectoryEntriesRequest {
                selection: Some(changed),
                at: first_page.resolved_at.clone(),
                page: Some(v1::PageRequest {
                    page_token: first_page.next_page_token.clone(),
                    ..Default::default()
                })
            }
        )
        .await
        .is_err()
    );
    fixture
        .runtime
        .operations
        .call_protobuf::<operations::DeleteSnapshot>(v1::StorageDeleteSnapshotRequest { snapshot: pinned.snapshot })
        .await
        .unwrap();
    assert!(
        super::transfer_plugin::client_call::<operations::ListDirectoryEntries>(
            &channel,
            v1::StorageListDirectoryEntriesRequest {
                selection: Some(selection.clone()),
                at: first_page.resolved_at,
                page: Some(v1::PageRequest {
                    page_token: first_page.next_page_token,
                    ..Default::default()
                })
            }
        )
        .await
        .is_err()
    );
    let fresh = super::transfer_plugin::client_call::<operations::ListDirectoryEntries>(
        &channel,
        v1::StorageListDirectoryEntriesRequest {
            selection: Some(selection.clone()),
            at: None,
            page: None,
        },
    )
    .await
    .unwrap();
    assert!(fresh.entries[0].path.ends_with("aa.bin"));
    attached.stop("filesystem metadata journey complete").await.unwrap();
    plugin.stop().await;
    drop(channel);
    listener.detach().await.unwrap();
    let data = fixture.data.clone();
    let recovery = fixture.recovery.clone();
    let settings = fixture.storage.settings().clone();
    let old_cut = fresh.resolved_at.unwrap();
    let old_token = second_page.next_page_token;
    fixture.finish().await;

    // Reopen The Same Persisted Provider State Under A New Read Owner.
    let storage = harmos_runtime::storage::StorageManager::new(
        harmos_storage::StorageBackends {
            primary: data.clone(),
            state: recovery.clone(),
        },
        settings,
    )
    .unwrap();
    let runtime = crate::support::app::launch::<super::Bank>(super::Ledger::default(), (), |setup| {
        setup.resource(std::ops::Deref::deref(&storage).clone());
        setup.resource(operations::SavedSnapshots::new(storage.query_capacity()));
        setup.settings().register(super::catalog()).operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(16 * 1024 * 1024).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    storage.start(&runtime).await.unwrap();
    crate::support::app::register(&runtime, operations::storage_catalog(), Default::default()).await.unwrap();
    assert!(
        runtime
            .operations
            .call_protobuf::<operations::ListDirectoryEntries>(v1::StorageListDirectoryEntriesRequest {
                selection: Some(selection.clone()),
                at: Some(old_cut.clone()),
                page: None,
            })
            .await
            .is_err()
    );
    assert!(
        runtime
            .operations
            .call_protobuf::<operations::ListDirectoryEntries>(v1::StorageListDirectoryEntriesRequest {
                selection: Some(selection.clone()),
                at: None,
                page: Some(v1::PageRequest {
                    page_token: old_token,
                    ..Default::default()
                }),
            })
            .await
            .is_err()
    );
    let reopened = runtime
        .operations
        .call_protobuf::<operations::ListDirectoryEntries>(v1::StorageListDirectoryEntriesRequest {
            selection: Some(selection),
            at: None,
            page: None,
        })
        .await
        .unwrap();
    assert_eq!(reopened.resolved_at.unwrap().positions, old_cut.positions);
    assert!(reopened.entries[0].path.ends_with("aa.bin"));
    storage.close().await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
}
