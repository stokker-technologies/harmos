/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Stream Publication Calls

/* === Modules === */

/* === Re-Exports === */

pub use harmos_operations::streams::PublishRequest;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::Call;
use crate::Result;
use harmos_operations::streams;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Streams<'a> {
    pub(super) call: &'a Call,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Streams<'_> {
    pub async fn publish(&self, request: PublishRequest) -> Result<()> {
        self.call.call::<streams::Publish>(request).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
