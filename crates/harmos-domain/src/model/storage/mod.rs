/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Values: Cursors, Keys, Selections, Query Plans, Files, Objects And
// Write Batches. The Runtime Owns Admission, Persistence And Execution.

/* === Modules === */
pub mod arrow;
pub mod codec;
pub mod data;
pub mod files;
pub mod follow;
pub mod keys;
pub mod mutations;
pub mod objects;
pub mod position;
pub mod query;
pub mod schema_input;
pub mod time;
pub mod writes;

/* === Re-Exports === */
pub use arrow::ArrowIpc;
pub use data::{RecordInput, RecordWrite, StoredRecord, StoredRow, StreamInput, StreamWrite};
pub use files::{
    ByteRange, FileBatch, FileCompletion, FileMetadata, FilePhase, FileProgress, FileReference,
    FileSession, ReadFile, StagedFile,
};
pub use follow::{File, Objects, Records, Stream};
pub use keys::{
    ObjectKey, ObjectRetention, ObjectValue, PayloadEncoding, SchemaKey, check_length, invalid,
    object_key,
};
pub use mutations::{FileProducer, MutationId, MutationWindow, StorageInfo};
pub use objects::{
    ObjectEntry, ObjectPoint, ObjectReceipt, ObjectRevision, ObjectVersion, ObjectVersionToken,
};
pub use position::{RecoveryPosition, StorageCursor, StoreHistory};
pub use query::{
    FileSelection, ObjectSelection, ObjectVersions, Query, QueryCompletion, QueryOutput,
    RecordSelection, RowSlice, SchemaSelection, Source, StreamSelection,
};
pub use schema_input::{SchemaInfo, SchemaInput};
pub use writes::{BatchReceipt, DataBatch, WriteCompletion, WriteReceipt, WriteRequest};

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
