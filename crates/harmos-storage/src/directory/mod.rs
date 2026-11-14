/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Charged Directory Discovery and Native Change Subscription

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
use crate::{Error, MemorySize, ReadBatch, Result, StorageManager};
use harmos_domain::model::storage::{DirectoryCompletion, DirectoryEntry, DirectoryEvent, DirectorySelection};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Selection {
    value: DirectorySelection,
    rules: selection::Rules,
}

/* === Enums === */

/* === Traits === */

trait DirectoryOutput: crate::MemorySize {
    fn output_bytes(&self) -> usize;
}
impl DirectoryOutput for DirectoryEntry {
    fn output_bytes(&self) -> usize {
        self.memory_size()
    }
}
impl DirectoryOutput for DirectoryEvent {
    fn output_bytes(&self) -> usize {
        self.memory_size()
    }
}

/* === Implementations === */

impl StorageManager {
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

fn item<T: DirectoryOutput>(inner: &Arc<Inner>, value: T) -> Result<ReadBatch<T>> {
    if value.output_bytes() > inner.settings.response.batch_bytes.get() {
        return Err(Error::ResourceExhausted { resource: "directory response bytes" });
    }
    Ok(ReadBatch::new(value))
}

async fn selection(mut value: DirectorySelection) -> Result<Arc<Selection>> {
    let path = value.root.clone();
    let context = crate::execution::Context::current();
    value.root =
        tokio::task::spawn_blocking(move || context.blocking(|| crate::local::validate_local_path(&path))).await.map_err(join_error)??;

    let rules = selection::Rules::compile(&value.rules)?;
    Ok(Arc::new(Selection { value, rules }))
}

fn relative(root: &Path, path: &Path, recursive: bool) -> Result<Option<String>> {
    let Ok(relative) = path.strip_prefix(root) else { return Ok(None) };
    if relative.as_os_str().is_empty() || (!recursive && relative.components().count() != 1) {
        return Ok(None);
    }
    let mut result = String::new();
    for component in relative.components() {
        let std::path::Component::Normal(value) = component else { return Ok(None) };
        let value = value.to_str().ok_or(Error::InvalidInput {
            field: "directory entry",
            reason: "path cannot be represented as UTF-8",
        })?;
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
