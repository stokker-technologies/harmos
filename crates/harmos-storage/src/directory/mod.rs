/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Charged Directory Discovery And Native Change Subscription

/* === Modules === */

mod scan;
mod selection;
mod watch;

/* === Re-Exports === */

pub use scan::DirectoryReader;
pub use watch::DirectoryWatcher;

/* === Imports === */

// Standard Library Imports
use std::path::Path;
use std::sync::Arc;
// Third Party Imports
// Local Imports
use crate::manager::Inner;
use crate::{Error, Message, ReadBatch, Result, StorageManager};
use harmos_domain::model::storage::{DirectoryCompletion, DirectoryEntry, DirectoryEvent, DirectorySelection};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Selection {
    value: DirectorySelection,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    pub async fn list_directory(&self, selection: DirectorySelection) -> Result<DirectoryReader> {
        let context = crate::execution::Context::current();
        let span = context.sync(|| tracing::debug_span!("storage.directory.list_directory", outcome = "cancelled"));
        context.result(span, async { DirectoryReader::new(Arc::clone(&self.inner), selection).await }).await
    }

    pub async fn watch_directory(&self, selection: DirectorySelection, include_existing: bool) -> Result<DirectoryWatcher> {
        let context = crate::execution::Context::current();
        let span = context.sync(|| tracing::debug_span!("storage.directory.watch_directory", outcome = "cancelled"));
        context.result(span, async { DirectoryWatcher::new(Arc::clone(&self.inner), selection, include_existing).await }).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn item<T: Message>(inner: &Arc<Inner>, value: T) -> Result<ReadBatch<T>> {
    if value.encoded_len() > inner.settings.response.batch_bytes.get() {
        return Err(Error::ResourceExhausted { resource: "directory response bytes" });
    }
    Ok(ReadBatch::new(value))
}

async fn selection(mut value: DirectorySelection) -> Result<Arc<Selection>> {
    let path = value.root.clone();
    let context = crate::execution::Context::current();
    value.root =
        tokio::task::spawn_blocking(move || context.blocking(|| crate::local::validate_local_path(&path))).await.map_err(join_error)??;

    Ok(Arc::new(Selection { value }))
}

fn relative(root: &Path, path: &Path, recursive: bool) -> Result<Option<String>> {
    let Ok(relative) = path.strip_prefix(root) else { return Ok(None) };
    if relative.as_os_str().is_empty() || (!recursive && relative.components().count() != 1) {
        return Ok(None);
    }
    let mut result = String::new();
    for component in relative.components() {
        let std::path::Component::Normal(value) = component else { return Ok(None) };
        let value =
            value.to_str().ok_or(Error::InvalidInput { field: "directory entry", reason: "path cannot be represented as UTF-8" })?;
        if !result.is_empty() {
            result.push('/');
        }
        result.push_str(value);
    }
    Ok(Some(result))
}

fn join_error(error: tokio::task::JoinError) -> Error {
    Error::backend("directory task", crate::BackendFailure::Permanent, error)
}

/* === Tests === */
