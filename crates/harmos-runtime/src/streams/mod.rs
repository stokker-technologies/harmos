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
mod ingest;

#[cfg(feature = "plugin")]
mod listeners;
mod manager;
mod publication;
mod publications;
mod reader;

#[cfg(feature = "plugin")]
mod registration;
mod series;

#[cfg(feature = "plugin")]
mod staging;
mod window;

/* === Re-Exports === */

pub use batches::{BatchItem, Batches, Measurement, Summary};
pub use definition::Stream;
pub use harmos_domain::streams::{Axial, BatchDefinition, Batched, Definition, Prefix, StreamCatalog, StreamRow, Track, Tracked};

#[cfg(feature = "plugin")]
pub use listeners::{PluginListenerBindings, PluginListenerRegistration};
pub use manager::{Query, Seal, StreamFeed, StreamManager, Survey};
pub use publication::ManagedPublication;
pub(crate) use publication::row_value;
pub use publications::{PluginStream, PublicationLease, PublicationRow, StreamAdmissionError, StreamPublicationError};
pub use reader::{PageLimits, ReadCut, ReadError, ReadLimits, RowPage, RowReader, RowSource};

#[cfg(feature = "plugin")]
pub use registration::StreamRegistration;

#[cfg(feature = "plugin")]
pub(crate) use staging::Activation;

#[cfg(feature = "plugin")]
pub use staging::{StagedPublication, StagedRows};

pub(crate) use definition::{Refused, Table, Wired};
pub use demand::Demand;
pub(crate) use demand::Holding;
pub use series::{Aggregated, Bucket, Point};
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
