/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod definition;
mod demand;
mod ingest;

mod connection;
#[cfg(feature = "plugin-guest")]
mod listeners;
mod manager;
mod publication;
mod publications;
pub(crate) use connection::ConnectionStreams;

#[cfg(feature = "plugin-guest")]
mod registration;

#[cfg(feature = "plugin-guest")]
mod staging;
mod window;

/* === Re-Exports === */

pub use definition::Stream;
pub use harmos_domain::streams::{Definition, StreamRow};

#[cfg(feature = "plugin-guest")]
pub use listeners::{PluginListenerBindings, PluginListenerRegistration};
pub use manager::{Query, StreamFeed, StreamManager};
pub use publication::ManagedPublication;
pub(crate) use publication::row_value;
pub use publications::{PublicationRow, PublishedStream, StreamPublicationError, StreamRegistrationError, StreamWriter};

#[cfg(feature = "plugin-guest")]
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
