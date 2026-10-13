/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod collection;

// Native Guest Hosting; Portable Authoring Lives In harmos-guest.
pub(crate) mod host;

/* === Re-Exports === */
pub use harmos_guest::operations;

pub use harmos_guest::operations::{CacheKey, OperationDeclaration, OperationPolicy, RetryPolicy};

pub use harmos_guest::{
    ConfigError, Plugin, Registrar, Registration, RegistrationError, StreamSpec, inventory,
    reserved,
};

pub use harmos_guest::{
    Context, OperationRefusal, PublishError, Publisher, UNKNOWN, UNREADABLE, WIT, export_plugin,
};

pub use collection::registrations;

pub use host::{Error, Host, Loaded, Registered, Serving};

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
