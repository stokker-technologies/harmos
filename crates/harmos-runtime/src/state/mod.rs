/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub(crate) mod capture;
mod entry {
    pub use harmos_domain::state::{
        At, CausationId, CorrelationId, Entry, Link, Metadata, Position, PrincipalId, Receipt, RequestKey, Timestamp,
    };
}
mod envelope;
mod history;
mod managed;
mod manager;
mod record;
pub(crate) mod recovery;
pub(crate) mod snapshot;
mod snapshot_codec;
mod transaction;
mod writer;

pub use crate::error::Error;
pub use capture::{CaptureError, CaptureLimits, JournalCapture};
pub use entry::{At, CausationId, CorrelationId, Entry, Link, Metadata, Position, PrincipalId, Receipt, RequestKey, Timestamp};
pub use envelope::{DefinitionId, Envelope, FactSource, Kind, Recorded, SystemFact};
pub use harmos_domain::state::{RecordDefinitions, TransactionDefinitions};
pub use history::{Focus, Origin, Reversals};
pub use managed::ManagedState;
pub use manager::StateManager;
pub(crate) use manager::{Context, Feed, TrackedState};
pub use record::{CatalogCodec, PreparationError, Prepared, Record, RecordCatalog, RecordProjection, Records};
pub use snapshot::SnapshotPolicy;
pub use transaction::{DecodeError, Invertible, Never, Transaction, TransactionCatalog};

// The proof traits generated authoring code reaches through `__private`: they
// are how a declared pair is checked mutual and how a declared inverse is
// checked to be a member of the catalog that will commit it. Public because an
// application's generated code names them, hidden because nothing else may.

pub use transaction::{Catalogued, Mutual, Paired};

// What boot needs of the journal's insides and nothing more: the fold replay
// rebuilds, the hook it re-derives inverses through, and the version table it
// dispatches stored entries by.
pub(crate) use history::{ReadRequest, Reads, ReplayPayload, Tail, Undoable as UndoEntry, Undos, replay};
pub(crate) use transaction::{Inverting, dispatch};
