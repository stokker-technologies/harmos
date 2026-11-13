/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Clients and Plugins Read Application-Owned Immutable Content.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime},
};

// Third Party Imports
use harmos_protocol::{context as child, storage as payload, v1};
use prost::Message;
use sha2::{Digest, Sha256};

// Local Imports
use harmos_operations::storage as operations;
use harmos_storage::{
    ContentLease, ContentSource, FileMetadata, FileReference, Identifier, StorageBackends, StorageCursor, StorageFuture, StoreHistory,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct ApplicationContent {
    bytes: &'static [u8],
    digest: [u8; 32],
    expires_at: SystemTime,
    resolutions: Mutex<Vec<(String, Option<String>)>>,
    maximum: AtomicUsize,
    corrupt: AtomicBool,
}
struct Authority;
struct ScopeMetadata;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ContentSource for ApplicationContent {
    fn resolve<'a>(&'a self, reference: &'a str, scope: Option<&'a str>) -> StorageFuture<'a, ContentLease> {
        Box::pin(async move {
            self.resolutions.lock().unwrap().push((reference.into(), scope.map(str::to_owned)));
            if reference != "application-content" || !matches!(scope, Some("client-scope" | "plugin-scope")) {
                return Err(harmos_storage::Error::InvalidInput {
                    field: "GetFileBytes",
                    reason: "application refused the reference or scope",
                });
            }
            Ok(ContentLease {
                length: self.bytes.len() as u64,
                sha256: self.digest,
                expires_at: self.expires_at,
                max_bytes: NonZeroUsize::new(self.maximum.load(Ordering::Acquire)).unwrap(),
                token: reference.as_bytes().to_vec(),
            })
        })
    }
    fn read<'a>(&'a self, lease: &'a ContentLease, offset: u64, maximum: NonZeroUsize) -> StorageFuture<'a, Vec<u8>> {
        Box::pin(async move {
            if lease.token != b"application-content" {
                return Err(harmos_storage::Error::InvalidInput {
                    field: "GetFileBytes",
                    reason: "application refused the lease",
                });
            }
            let offset = offset as usize;
            let end = offset.saturating_add(maximum.get().min(3)).min(self.bytes.len());
            let mut bytes = self.bytes[offset..end].to_vec();
            if self.corrupt.load(Ordering::Acquire) && offset == 0 && !bytes.is_empty() {
                bytes[0] ^= 1;
            }
            Ok(bytes)
        })
    }
}
impl harmos_runtime::operations::authorization::Authorizer for Authority {
    fn authorize(
        &self, _: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        Ok(Some(harmos_runtime::operations::authorization::Authorization::Restricted(
            harmos_runtime::operations::authorization::Required {
                contract: provider.contract.clone(),
                grants: ["application-content", "denied-content", "stored-content"]
                    .into_iter()
                    .map(|name| harmos_operations::plugin::Grant::of(provider.contract.operation().id.to_string(), name))
                    .collect(),
            },
        )))
    }
}
impl harmos_runtime::grpc::middleware::Middleware for ScopeMetadata {
    fn enter<'a>(&'a self, call: &'a mut harmos_runtime::grpc::middleware::Invocation) -> harmos_runtime::grpc::middleware::Enter<'a> {
        Box::pin(async move {
            if let Some(value) = call.metadata().get("x-scope").and_then(|value| value.to_str().ok()).map(str::to_owned) {
                call.context_mut().insert(harmos_runtime::Scope::new(value)?)?;
            }
            Ok(())
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn client_and_plugin_read_application_content_under_expiring_scoped_leases() {
    // Compose Storage and Its Application Source on One Controlled Clock.
    let clock = crate::support::clock::Controlled::new();
    let execution = clock.source();
    let data = Arc::new(super::Data::new(super::history()));
    let storage = harmos_runtime::storage::StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: Arc::new(super::Recovery::new(super::history())),
        },
        super::config(),
    )
    .unwrap();
    storage.adopt_clock(&execution).unwrap();
    let runtime = crate::support::app::launch::<super::Bank>(super::Ledger::default(), (), |setup| {
        setup.resource(std::ops::Deref::deref(&storage).clone());
        setup.resource(operations::SavedSnapshots::new(storage.query_capacity()));
        setup.settings().clock(execution.clone()).register(super::catalog()).operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(48 * 1024 * 1024).unwrap()),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    storage.start(&runtime).await.unwrap();
    crate::support::app::register(&runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    runtime.operations.register_authorizer(Arc::new(Authority)).unwrap();
    let bytes = b"immutable application-owned content";
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    let source = Arc::new(ApplicationContent {
        bytes,
        digest,
        expires_at: execution.read().date + Duration::from_secs(30),
        resolutions: Mutex::new(Vec::new()),
        maximum: AtomicUsize::new(5),
        corrupt: AtomicBool::new(false),
    });
    storage.register_content_source(source.clone()).unwrap();
    assert!(storage.register_content_source(source.clone()).is_err());
    let reference = FileReference {
        position: StorageCursor {
            history: StoreHistory {
                store: Identifier::new("application-store").unwrap(),
                epoch: Identifier::new("immutable-epoch").unwrap(),
            },
            sequence: 1,
        },
        metadata: FileMetadata {
            id: Identifier::new("application-content").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: Some(bytes.len() as u64),
            expected_sha256: Some(digest),
        },
        length: bytes.len() as u64,
        sha256: digest,
    };
    let file: payload::FileReference = reference.clone().into();
    // Retain a Library-Owned Saved File Beside the Application's Content.
    let mut saved = storage
        .create_file(FileMetadata {
            id: Identifier::new("stored-content").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: None,
            expected_sha256: None,
        })
        .await
        .unwrap();
    saved.write_all(b"library content".to_vec()).await.unwrap();
    let saved = saved.finish().await.unwrap();
    storage.flush_until(&saved.position).await.unwrap();
    let saved_before = storage.get_saved_position();
    // Expose Both Faces Through Real Sockets and the Existing Plugin Child Carrier.
    let plugin = super::transfer_plugin::TransferPlugin::start().await;
    let attached = runtime
        .attach_grpc(
            harmos_runtime::plugin::grpc::Placement::Remote(plugin.address.clone()),
            harmos_runtime::plugin::grpc::PlacementSettings {
                connect_timeout: Duration::from_secs(30),
                reconnect_delay: Duration::from_secs(1),
                stop_grace: Duration::from_secs(30),
                stream_window: NonZeroUsize::new(64).unwrap(),
            },
        )
        .await
        .unwrap();
    runtime.grpc().register_middleware(Arc::new(ScopeMetadata)).unwrap();
    let listener = runtime.grpc().attach(Default::default()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let maximum = harmos_protocol::transfer::default_message_bytes();
    let settings = harmos::client::ClientSettings::for_storage(
        storage.settings().transfer,
        maximum.checked_mul(NonZeroUsize::new(6).unwrap()).unwrap(),
    )
    .unwrap();
    let client = harmos::client::Client::new(channel, settings).unwrap();
    // Download All Client Ranges Under One Scoped Lease and Verify the Digest.
    let mut client_cut = None;
    let mut received = Vec::new();
    while received.len() < bytes.len() {
        let mut request = tonic::Request::new(v1::StorageGetFileBytesRequest {
            file: Some(file.clone()),
            offset: received.len() as u64,
            max_bytes: 7,
            at: client_cut.clone(),
        });
        request.metadata_mut().insert("x-scope", "client-scope".parse().unwrap());
        let range = client.storage.get_file_bytes(request).await.unwrap().into_inner();
        assert_eq!(range.total_length, bytes.len() as u64);
        assert_eq!(range.sha256, digest);
        assert!(!range.data.is_empty() && range.data.len() <= 5);
        assert!(range.resolved_at.is_some());
        received.extend(range.data);
        client_cut = range.resolved_at;
    }
    assert_eq!(received, bytes);
    assert_eq!(<[u8; 32]>::from(Sha256::digest(&received)), digest);
    // Download Through a Plugin, Preserving Its Distinct Inherited Authorization Scope.
    let plugin_calls = runtime.operations.with_scope("plugin-scope").unwrap();
    let mut plugin_cut = None;
    let mut received = Vec::new();
    while received.len() < bytes.len() {
        let request = child::StorageGetFileBytesRequest {
            file: Some(file.clone()),
            offset: received.len() as u64,
            max_bytes: 7,
            at: plugin_cut.clone(),
        };
        let response = plugin_calls.call_encoded("fixture.transfer.get_file_bytes", request.encode_to_vec()).await.unwrap();
        let range = child::StorageGetFileBytesResponse::decode(response.as_slice()).unwrap();
        assert_eq!(range.total_length, bytes.len() as u64);
        assert_eq!(range.sha256, digest);
        assert!(!range.data.is_empty() && range.data.len() <= 5);
        received.extend(range.data);
        plugin_cut = range.resolved_at;
    }
    assert_eq!(received, bytes);
    assert_eq!(<[u8; 32]>::from(Sha256::digest(&received)), digest);
    assert_eq!(
        *source.resolutions.lock().unwrap(),
        vec![("application-content".into(), Some("client-scope".into())), ("application-content".into(), Some("plugin-scope".into()))]
    );
    assert_eq!(storage.get_saved_position(), saved_before);
    assert!(!data.blobs.lock().unwrap().contains_key("application-content"));
    // Refuse a Different Scope Without Reusing Another Caller's Lease.
    let mut request = tonic::Request::new(v1::StorageGetFileBytesRequest {
        file: Some(file.clone()),
        offset: 0,
        max_bytes: 7,
        at: client_cut.clone(),
    });
    request.metadata_mut().insert("x-scope", "other-scope".parse().unwrap());
    assert!(client.storage.get_file_bytes(request).await.unwrap_err().message().contains("GetFileBytes"));
    // Preserve the Application's Refusal Rather Than Falling Back to Saved Storage.
    let mut denied = file.clone();
    denied.metadata.as_mut().unwrap().id = "denied-content".into();
    let mut request = tonic::Request::new(v1::StorageGetFileBytesRequest {
        file: Some(denied),
        offset: 0,
        max_bytes: 7,
        at: None,
    });
    request.metadata_mut().insert("x-scope", "client-scope".parse().unwrap());
    assert!(client.storage.get_file_bytes(request).await.unwrap_err().message().contains("GetFileBytes"));
    // Refuse Mismatched Metadata and Corruption Without Weakening the Digest Check.
    let mut changed = reference.clone();
    changed.length += 1;
    assert!(storage.open_content(&changed, Some("client-scope"), None).await.is_err());
    source.maximum.store(bytes.len(), Ordering::Release);
    source.corrupt.store(true, Ordering::Release);
    let read = storage.open_content(&reference, Some("client-scope"), None).await.unwrap();
    assert!(read.get_bytes(0, NonZeroUsize::new(bytes.len()).unwrap()).await.unwrap_err().to_string().contains("GetFileBytes"));
    source.corrupt.store(false, Ordering::Release);
    // Expire Both Issued Leases Without Resolving Them Again.
    let resolutions = source.resolutions.lock().unwrap().len();
    clock.advance(Duration::from_secs(30));
    let mut request = tonic::Request::new(v1::StorageGetFileBytesRequest {
        file: Some(file.clone()),
        offset: 0,
        max_bytes: 7,
        at: client_cut,
    });
    request.metadata_mut().insert("x-scope", "client-scope".parse().unwrap());
    assert!(client.storage.get_file_bytes(request).await.unwrap_err().message().contains("GetFileBytes"));
    let request = child::StorageGetFileBytesRequest {
        file: Some(file),
        offset: 0,
        max_bytes: 7,
        at: plugin_cut,
    };
    let error = plugin_calls.call_encoded("fixture.transfer.get_file_bytes", request.encode_to_vec()).await.unwrap_err();
    assert!(error.failure().unwrap().cause().contains("GetFileBytes"));
    assert_eq!(source.resolutions.lock().unwrap().len(), resolutions);
    // The Library's Own Saved Reference Still Reads After Application-Lease Expiry.
    let range = client
        .storage
        .get_file_bytes(tonic::Request::new(v1::StorageGetFileBytesRequest {
            file: Some(saved.into()),
            offset: 0,
            max_bytes: 0,
            at: None,
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(range.data, b"library content");
    assert_eq!(range.sha256, Sha256::digest(b"library content").to_vec());
    attached.stop("application content journey complete").await.unwrap();
    plugin.stop().await;
    listener.detach().await.unwrap();
    storage.close().await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
}
