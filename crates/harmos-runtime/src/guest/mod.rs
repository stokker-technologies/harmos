/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod collection;

// Native Guest Hosting; Portable Authoring Lives In harmos-guest.
pub(crate) mod host;

pub use harmos_macros::{guest_catalog as catalog, guest_operation as operation, plugin};

pub use harmos_guest::{kit, operations};

pub use harmos_guest::operations::{CacheKey, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy};

pub use harmos_guest::{ConfigError, Plugin, Registrar, Registration, RegistrationError, StreamSpec, inventory, reserved};

pub use harmos_guest::{Context, OperationRefusal, PublishError, Publisher, UNKNOWN, UNREADABLE, WIT, export_plugin};

pub use collection::{registrations, validate};

pub use host::{Error, Host, Loaded, Registered, Serving};
