/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Compose Query Persistence Alongside the Document's JSONL Recovery Adapter.

/* === Imports === */

use crate::app::SchematicEditor;
use harmos_runtime::{
    Setup,
    storage::{ObjectCatalog, StorageBackends, StorageManager, StoreHistory, config::StorageSettings},
};
use harmos_storage::{
    sqlite::{SqliteBackend, SqliteSettings},
    state::{StorageStateSettings, file::FileBackend},
};
use std::{path::Path, sync::Arc};

/* === Public Functions === */

pub fn attach(
    setup: &mut Setup<SchematicEditor>, root: &Path, history: StoreHistory, catalog: ObjectCatalog, settings: StorageSettings,
    recovery: StorageStateSettings,
) -> harmos_runtime::Result<StorageManager> {
    std::fs::create_dir_all(root).map_err(|source| harmos_runtime::Error::OperationFailed {
        operation: "create query storage".into(),
        source: Arc::new(source),
    })?;
    let primary = SqliteBackend::new(
        root.join("queries.sqlite"),
        history.clone(),
        SqliteSettings {
            chunk_rows: settings.response.batch_rows,
            chunk_bytes: settings.response.batch_bytes,
        },
    )?;
    let state = FileBackend::new(root.join("recovery"), history, recovery);
    let storage = StorageManager::with_catalog(
        catalog,
        StorageBackends {
            primary: Arc::new(primary),
            state: Arc::new(state),
        },
        settings,
    )?;
    setup.settings().storage(&storage);
    setup.catalog(harmos_runtime::builtins::storage_catalog(), harmos_runtime::operations::Lane::default());
    Ok(storage)
}
