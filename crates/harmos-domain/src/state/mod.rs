/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod entry;
mod envelope;
mod record;
mod record_registry;
mod state_catalog;
mod transaction;

/* === Re-Exports === */

pub use entry::{At, CausationId, CorrelationId, Entry, Link, Metadata, Position, PrincipalId, Receipt, RequestKey, Timestamp};
pub use envelope::{DefinitionId, Envelope, FactSource, Kind, SystemFact};
pub use record::{CatalogCodec, PreparationError, Prepared, Record, RecordEntry, Records};
pub use record_registry::RecordRegistry;
pub use state_catalog::{StateCatalog, StateDecoder, StateInverse, TransactionEntry};
pub use transaction::{DecodeError, Invertible, Never, Transaction};

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
