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

// Third Party Imports

// Local Imports
use super::CallContext;
use crate::Result;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[cfg(feature = "grpc")]
pub(crate) struct Deadline(pub(crate) tokio::time::Instant);

pub struct ContextMut<'a> {
    context: &'a mut CallContext,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<'a> ContextMut<'a> {
    pub(crate) fn new(context: &'a mut CallContext) -> Self {
        Self { context }
    }

    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) -> Result<()> {
        Ok(self.context.insert(value)?)
    }

    pub fn insert_portable<T: harmos_contract::operations::ContextValue>(
        &mut self,
        value: T,
    ) -> Result<()> {
        Ok(self.context.insert_portable(value)?)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
