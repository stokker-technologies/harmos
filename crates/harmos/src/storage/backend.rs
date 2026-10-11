/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::frame::{Scan, ScanCursor, ScanOrder, ScanPage};
use super::{
    BackendSnapshot, BootstrapId, BootstrapManifest, FileCheckpoint, FileMetadata, FileProgress,
    FileReference, MutationReceipt, ObjectBrowseCursor, ObjectBrowsePage, ObjectBrowseRequest,
    ObjectCatalog, ObjectEntry, ObjectPoint, ObjectVersions, QueryLimits, ReadCursor, ReadLimits,
    ReadPage, ReadSelection, ReadView, ReplicaTarget, RetainedBatch, RetainedSnapshot,
    SchemaEncoding, StagedFile, StorageBatch, StorageCursor, StorageSchema, StoreHistory,
    StoredRecord, StoredRow,
};

/* === Constants === */

/* === Macros === */
// Keep the physical method contract and private forwarding adapter in one list.
macro_rules! backend_contract {
    ($(#[$trait_doc:meta])* pub trait StorageBackend {
        $($(#[$doc:meta])* fn $method:ident $(<$lt:lifetime>)? (
            &$($borrow:lifetime)? self $(, $arg:ident: $ty:ty)* $(,)?
        ) -> $result:ty $(= $default:expr)?;)*
    }) => {
        $(#[$trait_doc])*
        pub trait StorageBackend: Send + Sync + 'static {
                        type Catalog: ObjectCatalog;
                                                fn schema_encoding(&self) -> SchemaEncoding { SchemaEncoding::Inline }
                                                            fn validate_object_browse(&self, _request: &ObjectBrowseRequest, _limits: QueryLimits) -> crate::Result<()> {
                Err(crate::Error::InvalidInput { field: "object browse", reason: "backend does not support scalar object browsing" })
            }
                                                            fn read_object_browse<'a>(&'a self, _view: &'a ReadView, _request: &'a ObjectBrowseRequest,
                _cursor: Option<&'a ObjectBrowseCursor>, _limits: ReadLimits) -> StorageFuture<'a, ObjectBrowsePage> {
                Box::pin(async { Err(crate::Error::InvalidInput { field: "object browse", reason: "backend does not support scalar object browsing" }) })
            }
                                                            fn validate_object_versions(&self, versions: ObjectVersions) -> crate::Result<()> {
                if versions == ObjectVersions::Latest {
                    return Err(crate::Error::InvalidInput {
                        field: "object query",
                        reason: "backend does not support latest object selection",
                    });
                }
                Ok(())
            }
                                                fn validate_query_limits(&self, _limits: QueryLimits) -> crate::Result<()> {
                Err(crate::Error::InvalidInput {
                    field: "query execution",
                    reason: "backend cannot enforce requested execution limits",
                })
            }
                                                fn read_query_view<'a>(&'a self, through: &'a StorageCursor, limits: QueryLimits) -> StorageFuture<'a, BackendSnapshot> {
                Box::pin(async move {
                    self.validate_query_limits(limits)?;
                    self.read_view(through).await
                })
            }
                                                fn supports_scan_order(&self, _scan: &Scan, order: &ScanOrder) -> bool {
                matches!(order, ScanOrder::Capture)
            }
                                                                                    fn scan_ordered<'a>(&'a self, _view: &'a ReadView, _scan: &'a Scan, _order: &'a ScanOrder,
                _cursor: Option<&'a ScanCursor>, _limits: ReadLimits) -> StorageFuture<'a, ScanPage> {
                Box::pin(async { Err(crate::Error::InvalidInput {
                    field: "storage frame",
                    reason: "backend does not support ordered frame scans",
                }) })
            }
            $(backend_contract!(@method $(#[$doc])* fn $method $(<$lt>)? (
                &$($borrow)? self $(, $arg: $ty)*
            ) -> $result $(= $default)?);)*
        }

        pub(super) trait ErasedBackend: Send + Sync + 'static {
            fn validate_object_browse(&self, request: &ObjectBrowseRequest, limits: QueryLimits) -> crate::Result<()>;
            fn read_object_browse<'a>(&'a self, view: &'a ReadView, request: &'a ObjectBrowseRequest,
                cursor: Option<&'a ObjectBrowseCursor>, limits: ReadLimits) -> StorageFuture<'a, ObjectBrowsePage>;
            fn validate_object_versions(&self, versions: ObjectVersions) -> crate::Result<()>;
            fn validate_query_limits(&self, limits: QueryLimits) -> crate::Result<()>;
            fn supports_scan_order(&self, scan: &Scan, order: &ScanOrder) -> bool;
            fn scan_ordered<'a>(&'a self, view: &'a ReadView, scan: &'a Scan, order: &'a ScanOrder,
                cursor: Option<&'a ScanCursor>, limits: ReadLimits) -> StorageFuture<'a, ScanPage>;
            fn read_query_view<'a>(&'a self, through: &'a StorageCursor, limits: QueryLimits) -> StorageFuture<'a, BackendSnapshot>;
            $(fn $method $(<$lt>)? (&$($borrow)? self $(, $arg: $ty)*) -> $result;)*
        }

        impl<C: ObjectCatalog> ErasedBackend for Adapter<C> {
            fn validate_object_browse(&self, request: &ObjectBrowseRequest, limits: QueryLimits) -> crate::Result<()> {
                self.0.validate_object_browse(request, limits)
            }
            fn read_object_browse<'a>(&'a self, view: &'a ReadView, request: &'a ObjectBrowseRequest,
                cursor: Option<&'a ObjectBrowseCursor>, limits: ReadLimits) -> StorageFuture<'a, ObjectBrowsePage> {
                self.0.read_object_browse(view, request, cursor, limits)
            }
            fn validate_object_versions(&self, versions: ObjectVersions) -> crate::Result<()> {
                self.0.validate_object_versions(versions)
            }
            fn supports_scan_order(&self, scan: &Scan, order: &ScanOrder) -> bool {
                self.0.supports_scan_order(scan, order)
            }
            fn scan_ordered<'a>(&'a self, view: &'a ReadView, scan: &'a Scan, order: &'a ScanOrder,
                cursor: Option<&'a ScanCursor>, limits: ReadLimits) -> StorageFuture<'a, ScanPage> {
                self.0.scan_ordered(view, scan, order, cursor, limits)
            }
            fn validate_query_limits(&self, limits: QueryLimits) -> crate::Result<()> {
                self.0.validate_query_limits(limits)
            }
            fn read_query_view<'a>(&'a self, through: &'a StorageCursor, limits: QueryLimits) -> StorageFuture<'a, BackendSnapshot> {
                self.0.read_query_view(through, limits)
            }
            $(fn $method $(<$lt>)? (&$($borrow)? self $(, $arg: $ty)*) -> $result {
                self.0.$method($($arg),*)
            })*
        }
    };
    (@method $(#[$doc:meta])* fn $method:ident $(<$lt:lifetime>)? (
        &$($borrow:lifetime)? self $(, $arg:ident: $ty:ty)*
    ) -> $result:ty = $default:expr) => {
        $(#[$doc])* fn $method $(<$lt>)? (&$($borrow)? self $(, $arg: $ty)*) -> $result {
            $default
        }
    };
    (@method $(#[$doc:meta])* fn $method:ident $(<$lt:lifetime>)? (
        &$($borrow:lifetime)? self $(, $arg:ident: $ty:ty)*
    ) -> $result:ty) => {
        $(#[$doc])* fn $method $(<$lt>)? (&$($borrow)? self $(, $arg: $ty)*) -> $result;
    };
}

backend_contract! {
pub trait StorageBackend {
    fn history(&self) -> &StoreHistory;
            fn open(&self) -> StorageFuture<'_, StorageCursor>;
    fn cursor_saved(&self) -> StorageFuture<'_, StorageCursor>;
                fn supports_retained_snapshots(&self) -> bool = false;
                        fn read_retained_view<'a>(&'a self, _through: &'a StorageCursor, _limits: QueryLimits) -> StorageFuture<'a, RetainedSnapshot> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "replica bootstrap", reason: "backend does not support retained snapshots" })
    });
                        fn begin_bootstrap<'a>(&'a self, _identity: &'a BootstrapId, _saved: &'a StorageCursor, _limits: QueryLimits) -> StorageFuture<'a, ()> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "replica bootstrap", reason: "backend does not support private import" })
    });
                fn write_bootstrap<'a>(&'a self, _batch: RetainedBatch<'a>) -> StorageFuture<'a, ()> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "replica bootstrap", reason: "backend does not support private import" })
    });
                    fn create_bootstrap_file<'a>(&'a self, _identity: &'a BootstrapId, _metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "replica bootstrap", reason: "backend does not support private file import" })
    });
            fn write_bootstrap_file<'a>(&'a self, _identity: &'a BootstrapId, _staging: &'a StagedFile, _offset: u64, _bytes: &'a [u8]) -> StorageFuture<'a, ()> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "replica bootstrap", reason: "backend does not support private file import" })
    });
            fn finish_bootstrap_file<'a>(&'a self, _identity: &'a BootstrapId, _staging: &'a StagedFile, _position: &'a StorageCursor, _length: u64, _sha256: [u8; 32]) -> StorageFuture<'a, FileReference> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "replica bootstrap", reason: "backend does not support private file import" })
    });
                fn abort_bootstrap_file<'a>(&'a self, _identity: &'a BootstrapId, _staging: &'a StagedFile) -> StorageFuture<'a, ()> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "replica bootstrap", reason: "backend does not support private file import" })
    });
                        fn finish_bootstrap<'a>(&'a self, _manifest: &'a BootstrapManifest) -> StorageFuture<'a, StorageCursor> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "replica bootstrap", reason: "backend does not support private import" })
    });
                    fn abort_bootstrap<'a>(&'a self, _identity: &'a BootstrapId) -> StorageFuture<'a, ()> = Box::pin(async { Ok(()) });
                                    fn write<'a>(&'a self, batch: StorageBatch<'a>) -> StorageFuture<'a, StorageCursor>;
                    fn read_view<'a>(&'a self, through: &'a StorageCursor) -> StorageFuture<'a, BackendSnapshot>;
                        fn release_view<'a>(&'a self, snapshot: &'a BackendSnapshot) -> StorageFuture<'a, ()>;
    fn read_schemas<'a>(
        &'a self,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<StorageSchema>>;
        fn read_files<'a>(
        &'a self,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<FileReference>>;
                    fn read_object<'a>(
        &'a self,
        view: &'a ReadView,
        query: &'a ObjectPoint,
        max_bytes: std::num::NonZeroUsize,
    ) -> StorageFuture<'a, Option<ObjectEntry>>;
                    fn read_objects<'a>(
        &'a self,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<ObjectEntry>>;
    fn read_records<'a>(
        &'a self,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<StoredRecord>>;
    fn read_stream<'a>(
        &'a self,
        view: &'a ReadView,
        selection: &'a ReadSelection,
        cursor: Option<&'a ReadCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ReadPage<StoredRow>>;
                fn supports_file_sessions(&self) -> bool = false;
        fn supports_mutation_receipts(&self) -> bool = false;
                fn save_mutations<'a>(&'a self, _receipts: &'a [MutationReceipt]) -> StorageFuture<'a, ()> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "storage mutations", reason: "backend does not support durable mutation receipts" })
    });
        fn recover_mutations(&self, _limits: ReadLimits) -> StorageFuture<'_, Vec<MutationReceipt>> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "storage mutations", reason: "backend does not support receipt recovery" })
    });
                    fn create_production<'a>(&'a self, _metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "file session", reason: "backend does not support resumable production" })
    });
                    fn fence_production<'a>(&'a self, _file: &'a StagedFile, _expected: u64, _next: u64) -> StorageFuture<'a, ()> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "file session", reason: "backend does not support durable fencing" })
    });
                    fn save_production<'a>(&'a self, _checkpoint: &'a FileCheckpoint) -> StorageFuture<'a, ()> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "file checkpoint", reason: "backend does not support production checkpoints" })
    });
                        fn recover_productions(&self, _limits: ReadLimits) -> StorageFuture<'_, Vec<FileCheckpoint>> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "file recovery", reason: "backend does not support production recovery" })
    });
                fn file_progress<'a>(&'a self, _file: &'a StagedFile) -> StorageFuture<'a, FileProgress> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "file following", reason: "backend does not support file progress" })
    });
                    fn read_production<'a>(&'a self, _file: &'a StagedFile, _offset: u64, _max_bytes: std::num::NonZeroUsize) -> StorageFuture<'a, Vec<u8>> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "file following", reason: "backend does not support production reads" })
    });
            fn supports_file_deletion(&self) -> bool = false;
                                    fn delete_file<'a>(&'a self, _file: &'a StagedFile) -> StorageFuture<'a, ()> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "file deletion", reason: "backend does not support deletion" })
    });
                        fn abandon_file<'a>(&'a self, _staging: &'a StagedFile) -> StorageFuture<'a, ()> = Box::pin(async {
        Err(crate::Error::InvalidInput { field: "file session", reason: "backend does not support retained abandonment" })
    });
                        fn create_file<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile>;
            fn write_file<'a>(
        &'a self,
        staging: &'a StagedFile,
        offset: u64,
        bytes: &'a [u8],
    ) -> StorageFuture<'a, ()>;
                                            fn finish_file<'a>(
        &'a self,
        staging: &'a StagedFile,
        position: &'a StorageCursor,
        length: u64,
        sha256: [u8; 32],
    ) -> StorageFuture<'a, FileReference>;
            fn abort_file<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()>;
                            fn read_file<'a>(
        &'a self,
        view: &'a ReadView,
        file: &'a FileReference,
        offset: u64,
        max_bytes: std::num::NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>>;
                                                fn prune_objects<'a>(
        &'a self,
        through: &'a StorageCursor,
        max_entries: std::num::NonZeroUsize,
    ) -> StorageFuture<'a, bool>;
    fn close(&self) -> StorageFuture<'_, ()>;
}

}

/* === Type Aliases === */
pub type StorageFuture<'a, T> = Pin<Box<dyn Future<Output = crate::Result<T>> + Send + 'a>>;

/* === Structs === */
pub struct StorageBackends<C: ObjectCatalog = ()> {
    pub primary: Arc<dyn StorageBackend<Catalog = C>>,
    pub recovery: Arc<dyn super::recovery::RecoveryBackend>,
    pub replicas: Vec<ReplicaTarget<C>>,
}

struct Adapter<C: ObjectCatalog>(Arc<dyn StorageBackend<Catalog = C>>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(super) fn erase<C: ObjectCatalog>(
    backend: Arc<dyn StorageBackend<Catalog = C>>,
) -> Arc<dyn ErasedBackend> {
    Arc::new(Adapter(backend))
}

/* === Private Functions === */

/* === Tests === */
