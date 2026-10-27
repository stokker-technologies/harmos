/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::Bindings;

use crate::Result;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Register<H> = fn(&mut Bindings<'_>, &'static str, Arc<H>) -> Result<()>;

/* === Structs === */

pub struct Binding<H> {
    pub(super) register: Register<H>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<H> Binding<H> {
    pub(super) fn apply(&self, bindings: &mut Bindings<'_>, lane: &'static str, handler: Arc<H>) -> Result<()> {
        (self.register)(bindings, lane, handler)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
