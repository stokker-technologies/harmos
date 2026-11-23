/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub(crate) mod protobuf;

mod definition;
mod demand;
mod ingest;

#[cfg(feature = "plugin-guest")]
mod listeners;
mod manager;
mod publication;
mod publications;

mod registration;

#[cfg(feature = "plugin-guest")]
mod staging;
pub(crate) mod subscription;
mod window;

/* === Re-Exports === */

pub use harmos_domain::streams::{Definition, StreamEntry, StreamRow};

#[cfg(feature = "plugin-guest")]
pub use listeners::{PluginListenerBindings, PluginListenerRegistration};
pub(crate) use manager::Captures;
pub use manager::{CapturingExecutions, Query, StreamFeed, StreamManager};
pub(crate) use publication::Missed;
pub use publication::{Gap, ManagedPublication};
pub(crate) use publication::{RETAINED_BYTES, RETAINED_ROWS, claim, row_value};
pub use publications::{PublicationRow, PublishedStream, StreamPublicationError, StreamRegistrationError, StreamWriter};

pub use registration::StreamRegistration;

#[cfg(feature = "plugin-guest")]
pub(crate) use staging::Activation;

#[cfg(feature = "plugin-guest")]
pub use staging::{StagedPublication, StagedRows};

pub(crate) use definition::{Refused, Table, Wired};
pub use demand::Demand;
pub(crate) use demand::Holding;
pub use window::{BeneathWindow, Item};

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
