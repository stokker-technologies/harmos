/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Re-Exports === */

pub use harmos_operations::lifecycle::{Cancel, CancelRequest, InstanceState, Status, StatusRequest, Watch, WatchRequest};

/* === Structs === */

pub struct Catalog;

/* === Implementations === */

impl Catalog {
    pub fn registration() -> harmos_sdk::RegistrationBatch {
        harmos_operations::lifecycle::Lifecycle::registration(harmos_sdk::Dependencies::new())
    }
}

pub fn catalog_registration<C: 'static>(
    storage: Option<std::sync::Arc<crate::storage::StorageManager<C>>>,
) -> crate::Result<harmos_sdk::RegistrationBatch> {
    let mut dependencies = harmos_sdk::Dependencies::new();
    let storage = storage.map(|storage| std::sync::Arc::new(storage.engine_erased()));
    dependencies.insert(std::sync::Arc::new(storage))?;
    Ok(harmos_operations::host::Host::registration(dependencies))
}
