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
use harmos_runtime::builtins;
use harmos_runtime::operations::Lane;
use harmos_runtime::{
    Result, Setup,
    storage::{ObjectCatalog, StorageManager, StorageSettings},
};
use schematic_operations::transfer;

/* === Public Functions === */

pub fn attach(setup: &mut Setup<SchematicEditor>, catalog: ObjectCatalog, settings: StorageSettings) -> Result<StorageManager> {
    let storage = StorageManager::in_memory(catalog, settings)?;
    setup.settings().storage(&storage);
    crate::storage::recordings(setup, storage.clone());
    setup.catalog(builtins::storage_catalog(), Lane::default());
    setup.catalog(transfer::files_catalog(), Lane::default());
    Ok(storage)
}
