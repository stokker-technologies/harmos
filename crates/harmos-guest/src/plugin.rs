/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod error;
mod owner;

/* === Re-Exports === */

pub use error::Error;
pub use harmos_domain::plugin::*;
pub(crate) use owner::Owner;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::dispatch::Table;
use harmos_domain::model::operations::lifecycle;
use harmos_operations::execution::operations::ServingError;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub trait Context: harmos_operations::execution::lifecycle::Context<Configuration = Table> {
    fn assign_operations(&self, bytes: &[u8]) -> Result<(), ServingError>;
    fn with_generation(&self, token: [u8; 32]) -> Self;
}

pub trait Plugin: harmos_operations::execution::Lifecycle<Context: Context> + lifecycle::Policy + Sync {
    const CATALOGS: &'static [harmos_operations::execution::operations::Catalog];

    fn register(_registrar: &mut harmos_operations::execution::Registrar) -> Result<(), harmos_operations::execution::RegistrationError> {
        Ok(())
    }
}

/* === Implementations === */

/* === Public Functions === */
pub(crate) fn bind<P: crate::plugin::Plugin>(
    owner: std::sync::Arc<crate::plugin::Owner<P>>, table: &std::sync::Arc<std::sync::Mutex<crate::dispatch::Table>>,
) -> Result<(), crate::plugin::Error<P>> {
    use crate::lifecycle::access::LifecycleAccess;
    let mut dependencies = harmos_operations::execution::__private::ResourceSet::new();
    dependencies
        .insert(std::sync::Arc::new(LifecycleAccess(owner)))
        .map_err(|error| crate::plugin::Error::Resource(std::sync::Arc::new(error)))?;
    let mut table = table.lock().map_err(|_| crate::plugin::Error::Poisoned)?;
    let initialize = harmos_operations::lifecycle::Descriptor::of_initialize::<P>()
        .map_err(harmos_operations::execution::ServingError::from)
        .map_err(|error| crate::plugin::Error::Serving(std::sync::Arc::new(error)))?;
    let shutdown = harmos_operations::lifecycle::Descriptor::of_shutdown::<P>()
        .map_err(harmos_operations::execution::ServingError::from)
        .map_err(|error| crate::plugin::Error::Serving(std::sync::Arc::new(error)))?;
    table
        .install_lifecycle::<crate::lifecycle::Initialize>(dependencies.clone(), initialize)
        .map_err(|error| crate::plugin::Error::Serving(std::sync::Arc::new(error)))?;
    table
        .install_lifecycle::<crate::lifecycle::Shutdown>(dependencies, shutdown)
        .map_err(|error| crate::plugin::Error::Serving(std::sync::Arc::new(error)))
}

/* === Private Functions === */

/* === Tests === */
