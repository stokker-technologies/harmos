/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::sync::Arc;

use super::Bindings;

use crate::Result;

type Register<H> = fn(&mut Bindings<'_>, &'static str, Arc<H>) -> Result<()>;

pub struct Binding<H> {
    pub(super) register: Register<H>,
}

impl<H> Binding<H> {
    pub(super) fn apply(&self, bindings: &mut Bindings<'_>, lane: &'static str, handler: Arc<H>) -> Result<()> {
        (self.register)(bindings, lane, handler)
    }
}
