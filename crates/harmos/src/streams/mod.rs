/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod batches;
mod definition;
mod demand;

#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
mod listeners;
mod manager;
mod publication;
mod publications;
mod reader;

#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
mod registration;
mod series;

#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
mod staging;
mod window;

/* === Re-Exports === */
pub use batches::{BatchItem, Batches, Measurement, Statistics, Summary};
pub use definition::Stream;

#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
pub use listeners::{PluginListenerBindings, PluginListenerRegistration};
pub use manager::{Query, Seal, StreamFeed, StreamManager, Survey};
pub use publication::ManagedPublication;
pub(crate) use publication::row_value;
pub use publications::{
    PluginStream, PublicationLease, PublicationRow, StreamAdmissionError, StreamPublicationError,
};
pub use reader::{PageLimits, ReadCut, ReadError, ReadLimits, RowPage, RowReader, RowSource};

#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
pub use registration::StreamRegistration;

#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
pub(crate) use staging::Activation;

#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
pub use staging::StagedPublication;

// Declared beside the row type rather than beside the machinery that serves
// one. The crate root hands them out from there; this is what lets the modules
// below go on reaching them through `super::`.
pub(crate) use crate::declaration::{Axial, BatchDefinition, Batched, Definition, Prefix, Tracked};
pub(crate) use definition::{Refused, Table, Wired};
pub use demand::Demand;
pub(crate) use demand::Holding;
pub use series::{Aggregate, Aggregated, Bucket, Point};
pub(crate) use series::{Opened, Request, Tier};
pub use window::{BeneathWindow, Item};

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use manager::Partition;

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
