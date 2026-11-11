/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod cancel;
mod initialize;
mod shutdown;
mod status;
mod watch;

/* === Re-Exports === */

pub use cancel::Cancel;
pub use harmos_domain::model::operations::lifecycle::*;

#[cfg(feature = "lifecycle")]
pub use initialize::ControlError;
pub use initialize::Initialize;
pub use shutdown::Shutdown;
pub use status::{InstanceState, Status};
pub use watch::Watch;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

pub const POLICY: harmos_domain::OperationPolicy = harmos_domain::OperationPolicy {
    timeout: std::time::Duration::from_secs(30),
    attempt_timeout: std::time::Duration::from_secs(30),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_domain::RetryPolicy::Never,
};
const WATCH_POLICY: harmos_domain::OperationPolicy = harmos_domain::OperationPolicy {
    timeout: harmos_domain::operations::UNBOUNDED,
    attempt_timeout: harmos_domain::operations::UNBOUNDED,
    cancellation_timeout: std::time::Duration::ZERO,
    ..POLICY
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[harmos::catalog(harmos_path = crate)]
pub enum Lifecycle {
    Status(Status),
    Cancel(Cancel),
    Watch(Watch),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[cfg(feature = "lifecycle")]
pub fn bind<P: crate::execution::Plugin>(
    owner: std::sync::Arc<crate::execution::plugin::Owner<P>>, table: &crate::execution::plugin::Table,
) -> Result<(), crate::execution::plugin::Error<P>> {
    use crate::execution::operations::lifecycle::LifecycleAccess;
    let mut dependencies = crate::execution::__private::ResourceSet::new();
    dependencies
        .insert(std::sync::Arc::new(LifecycleAccess(owner)))
        .map_err(|error| crate::execution::plugin::Error::Resource(std::sync::Arc::new(error)))?;
    let mut table = table.lock().map_err(|_| crate::execution::plugin::Error::Poisoned)?;
    let initialize = Descriptor::of_initialize::<P>()
        .map_err(crate::execution::ServingError::from)
        .map_err(|error| crate::execution::plugin::Error::Serving(std::sync::Arc::new(error)))?;
    let shutdown = Descriptor::of_shutdown::<P>()
        .map_err(crate::execution::ServingError::from)
        .map_err(|error| crate::execution::plugin::Error::Serving(std::sync::Arc::new(error)))?;
    table
        .install_lifecycle::<Initialize, initialize::Control>(dependencies.clone(), initialize)
        .map_err(|error| crate::execution::plugin::Error::Serving(std::sync::Arc::new(error)))?;
    table
        .install_lifecycle::<Shutdown, shutdown::Control>(dependencies, shutdown)
        .map_err(|error| crate::execution::plugin::Error::Serving(std::sync::Arc::new(error)))
}

/* === Private Functions === */

/* === Tests === */
