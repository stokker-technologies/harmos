/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod catalog;
mod entry;
mod envelope;
mod record;
mod transaction;

/* === Re-Exports === */
pub use catalog::{RecordDefinitions, TransactionDefinitions};
pub use entry::{
    At, CausationId, CorrelationId, Entry, Link, Metadata, Position, PrincipalId, Receipt,
    RequestKey, Timestamp,
};
pub use envelope::{DefinitionId, Envelope, FactSource, Kind, SystemFact};
pub use record::{
    CatalogCodec, PreparationError, Prepared, Record, RecordCatalog, RecordProjection, Records,
};
pub use transaction::{
    Catalogued, DecodeError, Invert, Mutual, Never, Transaction, TransactionCatalog, Undoable,
};
