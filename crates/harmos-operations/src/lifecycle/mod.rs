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
pub fn bind<P: harmos_sdk::Plugin>(
    owner: std::sync::Arc<harmos_sdk::plugin::Owner<P>>, table: &harmos_sdk::plugin::Table,
) -> Result<(), harmos_sdk::plugin::Error<P>> {
    use harmos_sdk::operations::lifecycle::LifecycleAccess;
    let mut dependencies = harmos_sdk::__private::ResourceSet::new();
    dependencies
        .insert(std::sync::Arc::new(LifecycleAccess(owner)))
        .map_err(|error| harmos_sdk::plugin::Error::Resource(std::sync::Arc::new(error)))?;
    let mut table = table.lock().map_err(|_| harmos_sdk::plugin::Error::Poisoned)?;
    let initialize = Descriptor::of_initialize::<P>()
        .map_err(harmos_sdk::ServingError::from)
        .map_err(|error| harmos_sdk::plugin::Error::Serving(std::sync::Arc::new(error)))?;
    let shutdown = Descriptor::of_shutdown::<P>()
        .map_err(harmos_sdk::ServingError::from)
        .map_err(|error| harmos_sdk::plugin::Error::Serving(std::sync::Arc::new(error)))?;
    table
        .install_lifecycle::<Initialize, initialize::Control>(dependencies.clone(), initialize)
        .map_err(|error| harmos_sdk::plugin::Error::Serving(std::sync::Arc::new(error)))?;
    table
        .install_lifecycle::<Shutdown, shutdown::Control>(dependencies, shutdown)
        .map_err(|error| harmos_sdk::plugin::Error::Serving(std::sync::Arc::new(error)))
}

/* === Private Functions === */

/* === Tests === */
