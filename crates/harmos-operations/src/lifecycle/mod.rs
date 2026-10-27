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

pub use cancel::{Cancel, CancelRequest};
pub use harmos_domain::model::operations::lifecycle::*;
pub use initialize::{Initialize, InitializeRequest};
pub use shutdown::{Shutdown, ShutdownRequest};
pub use status::{InstanceState, Status, StatusRequest};
pub use watch::{Watch, WatchRequest};

/* === Constants === */

pub const POLICY: harmos_domain::OperationPolicy = harmos_domain::OperationPolicy {
    timeout: std::time::Duration::from_secs(30),
    attempt_timeout: std::time::Duration::from_secs(30),
    retries: harmos_domain::RetryPolicy::Never,
};
const WATCH_POLICY: harmos_domain::OperationPolicy = harmos_domain::OperationPolicy {
    timeout: harmos_domain::operations::UNBOUNDED,
    attempt_timeout: harmos_domain::operations::UNBOUNDED,
    ..POLICY
};

/* === Enums === */

#[harmos::catalog(harmos_path = crate)]
pub enum Lifecycle {
    Status(Status),
    Cancel(Cancel),
    Watch(Watch),
}

#[cfg(feature = "runtime")]
pub fn bind<P: harmos_sdk::Plugin>(
    owner: std::sync::Arc<harmos_sdk::plugin::Owner<P>>, table: &harmos_sdk::plugin::Table,
) -> Result<(), harmos_sdk::plugin::Error<P>> {
    use harmos_sdk::operations::lifecycle::LifecycleAccess;
    let mut dependencies = harmos_sdk::Dependencies::new();
    dependencies
        .insert(std::sync::Arc::new(LifecycleAccess(owner)))
        .map_err(|error| harmos_sdk::plugin::Error::Dependency(std::sync::Arc::new(error)))?;
    let mut table = table.lock().map_err(|_| harmos_sdk::plugin::Error::Poisoned)?;
    let initialize = Descriptor::of_initialize::<P>()
        .map_err(harmos_sdk::ServingError::from)
        .map_err(|error| harmos_sdk::plugin::Error::Serving(std::sync::Arc::new(error)))?;
    let shutdown = Descriptor::of_shutdown::<P>()
        .map_err(harmos_sdk::ServingError::from)
        .map_err(|error| harmos_sdk::plugin::Error::Serving(std::sync::Arc::new(error)))?;
    table
        .install_lifecycle::<Initialize, Initialize>(dependencies.clone(), initialize)
        .map_err(|error| harmos_sdk::plugin::Error::Serving(std::sync::Arc::new(error)))?;
    table
        .install_lifecycle::<Shutdown, Shutdown>(dependencies, shutdown)
        .map_err(|error| harmos_sdk::plugin::Error::Serving(std::sync::Arc::new(error)))
}
