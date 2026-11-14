/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Compose The In-Memory Storage Adapter With Native Operations.

/* === Imports === */

use crate::app::SchematicEditor;
use harmos_runtime::{Setup, config::ManagedStorageSettings, storage::{ObjectCatalog, ReadLimits, StorageManager, StorageSettings}};

/* === Public Functions === */

pub fn attach(setup: &mut Setup<SchematicEditor>, catalog: ObjectCatalog, settings: StorageSettings) -> harmos_runtime::Result<StorageManager> {
    let limits = ReadLimits { rows: settings.response.batch_rows, bytes: settings.response.batch_bytes };
    let storage = StorageManager::in_memory(catalog, settings)?;
    setup.settings().storage(&storage).managed_storage(ManagedStorageSettings {
        replay: limits, capture: limits, snapshots: None, journal_tail: 128,
    });
    crate::storage::recordings(setup, storage.clone());
    setup.catalog(harmos_runtime::builtins::storage_catalog(), harmos_runtime::operations::Lane::default());
    setup.catalog(schematic_operations::transfer::files_catalog(), harmos_runtime::operations::Lane::default());
    Ok(storage)
}
