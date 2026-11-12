/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod declaration;

// Native Guest Hosting; Portable Authoring Lives In the SDK Wasm Adapter.
pub(crate) mod host;

/* === Re-Exports === */

pub use host::plugin::*;

pub use declaration::Declaration;

pub use host::{Error, Host, Loaded, Registered, Serving, inspect};

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
