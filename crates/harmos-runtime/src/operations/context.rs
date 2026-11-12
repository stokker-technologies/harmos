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

/// Opaque Scope Inherited by Host and Plugin Child Calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope(pub(crate) String);

pub struct ContextMut<'a> {
    context: &'a mut CallContext,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Scope {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'a> ContextMut<'a> {
    pub(crate) fn new(context: &'a mut CallContext) -> Self {
        Self { context }
    }

    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) -> Result<()> {
        Ok(self.context.insert(value)?)
    }

    pub fn insert_portable<T: harmos_operations::operations::ContextValue>(&mut self, value: T) -> Result<()> {
        Ok(self.context.insert_portable(value)?)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
