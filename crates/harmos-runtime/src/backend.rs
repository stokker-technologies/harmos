/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Backend Authoring Contracts

//! Implement Storage and Recovery Backends Without Depending on Internal Crates.
//!
//! This Barrel Exposes Backend Contracts and Their Values, Excluding Runtime Managers.
//!
//! ```compile_fail,E0432
//! use harmos_runtime::backend::StorageManager;
//! ```

/* === Modules === */

pub mod query;
pub mod validation;

/* === Re-Exports === */

pub use harmos_domain::model::storage::{IdentitySelection, StoredIdentity};
pub use harmos_domain::state::Position;
pub use harmos_storage::memory::{Backend, RecoveryBackend};
pub use harmos_storage::query::columnar::StorageCapabilities;
pub use harmos_storage::state::{
    RecoveryEntry, RecoveryPage, RecoverySnapshot, Segment, SnapshotManifest, SnapshotPayload, SnapshotWriter, StateBackend,
};
pub use harmos_storage::writes::{DataBatch, DecodedBatch, SampleAppend};
pub use harmos_storage::{
    BackendFailure, BackendSnapshot, CompareExchange, CompoundExchange, CompoundOutcome, CompoundReceipt, CompoundWrite, DefinitionInput,
    DefinitionKind, DefinitionOutcome, DefinitionRefusal, Error, FileCheckpoint, FileChunk, FileMetadata, FileProgress, FileReference,
    Identifier, MemorySize, MutationHeader, MutationId, MutationItem, MutationReceipt, MutationValue, MutationWindow, ObjectCatalog,
    ObjectDefinition, ObjectEntry, ObjectExchange, ObjectHandle, ObjectKey, ObjectPoint, ObjectRetention, ObjectValue, ObjectVersion,
    ObjectVersionToken, PayloadEncoding, ReadLimits, RecordInput, RecordView, RecoveryPosition, Result, ScanPage, SchemaExchange,
    SchemaKey, SchemaWrite, SchemaWriteReceipt, StagedFile, StorageBackend, StorageBackends, StorageBatch, StorageCursor,
    StorageFilesystemSettings, StorageFuture, StoragePolicy, StorageRequestSettings, StorageResponseSettings, StorageSchema,
    StorageSettings, StorageWork, StoreHistory, StoredObject, StoredRecord, StoredRow, StreamInput, encoded_size,
};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

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
