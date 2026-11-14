/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Wasm Child Output Boundary

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::Result;
use harmos_protocol::plugin::instance_invoke_response;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

// Guest contexts use ChildTransport; a native frame output cannot be constructed.
#[derive(Clone)]
pub(crate) enum ChildOutput {}

/* === Traits === */

/* === Implementations === */

impl ChildOutput {
    pub(super) fn check(&self) -> Result<()> {
        match *self {}
    }

    pub(super) async fn send(&self, _: instance_invoke_response::Frame) -> Result<()> {
        match *self {}
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
