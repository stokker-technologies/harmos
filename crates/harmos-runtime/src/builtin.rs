/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Re-Exports === */

/* === Structs === */

pub fn catalog_registration<C: 'static>(
    storage: Option<std::sync::Arc<crate::storage::StorageManager<C>>>,
) -> crate::Result<harmos_sdk::RegistrationBatch> {
    let mut dependencies = harmos_sdk::Dependencies::new();
    let storage = storage.map(|storage| std::sync::Arc::new(storage.engine_erased()));
    dependencies.insert(std::sync::Arc::new(storage))?;
    Ok(harmos_operations::host::Host::registration(dependencies))
}

pub fn storage_registration<C: 'static>(
    storage: std::sync::Arc<crate::storage::StorageManager<C>>,
) -> crate::Result<harmos_sdk::RegistrationBatch> {
    let mut dependencies = harmos_sdk::Dependencies::new();
    dependencies.insert(std::sync::Arc::new(storage.engine_erased()))?;
    Ok(harmos_operations::storage::Storage::registration(dependencies))
}
