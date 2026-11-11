/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod owner;

/* === Re-Exports === */

pub use crate::execution::error::Error;
pub use harmos_domain::plugin::*;
pub use owner::{Owner, Table};

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::execution::operations::{ServingError, WireTable};
use harmos_domain::model::operations::lifecycle;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub trait Context: crate::execution::lifecycle::Context<Configuration = WireTable> {
    fn assign_operations(&self, bytes: &[u8]) -> Result<(), ServingError>;
    fn with_generation(&self, token: [u8; 32]) -> Self;
}

pub trait Plugin: crate::execution::Lifecycle<Context: Context> + lifecycle::Policy + Sync {
    const CATALOGS: &'static [crate::execution::operations::Catalog];

    fn register(_registrar: &mut crate::execution::Registrar) -> Result<(), crate::execution::RegistrationError> {
        Ok(())
    }

    fn attachment(&self) -> Option<Arc<dyn Attachment<Self::Context>>> {
        None
    }
}

pub trait Attachment<C>: Send + Sync + 'static {
    #[doc(hidden)]
    fn bind(self: Arc<Self>, table: &mut WireTable) -> Result<(), ServingError>;

    fn observers(&self) -> Vec<crate::execution::inventory::observers::Declaration>;
    fn quiesce(&self) -> Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + '_>>;
}

/* === Implementations === */

impl crate::execution::lifecycle::Context for () {
    type Configuration = WireTable;
}

impl Context for () {
    fn assign_operations(&self, bytes: &[u8]) -> Result<(), ServingError> {
        if bytes.is_empty() { Ok(()) } else { Err(ServingError::Catalog) }
    }
    fn with_generation(&self, _: [u8; 32]) -> Self {}
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
