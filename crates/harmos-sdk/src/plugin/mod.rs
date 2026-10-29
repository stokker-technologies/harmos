/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod owner;

pub use crate::error::Error;
pub use harmos_domain::plugin::*;
pub use owner::{Owner, Table};

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::operations::{ServingError, WireTable};
use harmos_domain::model::operations::lifecycle;

pub trait Context: crate::lifecycle::Context<Configuration = WireTable> {
    fn assign_operations(&self, bytes: &[u8]) -> Result<(), ServingError>;
    fn with_generation(&self, token: [u8; 32]) -> Self;
}

pub trait Plugin: crate::Lifecycle<Context: Context> + lifecycle::Policy + Sync {
    const CATALOGS: &'static [crate::operations::Catalog];

    fn register(_registrar: &mut crate::Registrar) -> Result<(), crate::RegistrationError> {
        Ok(())
    }

    fn attachment(&self) -> Option<Arc<dyn Attachment<Self::Context>>> {
        None
    }
}

pub trait Attachment<C>: Send + Sync + 'static {
    #[doc(hidden)]
    fn bind(self: Arc<Self>, table: &mut WireTable) -> Result<(), ServingError>;

    fn observers(&self) -> Vec<crate::inventory::observers::Declaration>;
    fn quiesce(&self) -> Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + '_>>;
}

impl crate::lifecycle::Context for () {
    type Configuration = WireTable;
}

impl Context for () {
    fn assign_operations(&self, bytes: &[u8]) -> Result<(), ServingError> {
        if bytes.is_empty() { Ok(()) } else { Err(ServingError::Catalog) }
    }
    fn with_generation(&self, _: [u8; 32]) -> Self {}
}
