/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{
    columnar::{Cancellation, StorageCapabilities},
    index::IndexDefinition,
    projection::{ProjectionDefinition, ProjectionPage, ProjectionRead, ProjectionStatus},
};
use crate::{Identifier, ReadLimits, Result, StorageCursor, StorageManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StorageManager {
    pub fn query_capacity(&self) -> std::num::NonZeroUsize {
        self.inner.settings.request.max_active_queries
    }
    pub async fn get_index_definition(&self, name: &Identifier) -> Result<(IndexDefinition, u64)> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.get_index_definition", self.inner.primary.get_index_definition(name)).await
    }
    pub async fn get_projection_definition(&self, name: &Identifier, revision: Option<u64>) -> Result<(ProjectionDefinition, u64)> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.get_projection_definition", self.inner.primary.get_projection_definition(name, revision)).await
    }
    pub fn get_storage_capabilities(&self) -> StorageCapabilities {
        self.inner.primary.get_storage_capabilities()
    }
    pub async fn set_index_definition(
        &self, definition: &IndexDefinition, rebuild: bool, work: ReadLimits, cancel: &Cancellation,
    ) -> Result<u64> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.set_index_definition", self.inner.primary.set_index_definition(definition, rebuild, work, cancel)).await
    }
    pub async fn set_projection_definition(&self, definition: &ProjectionDefinition, rebuild: bool) -> Result<u64> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.set_projection_definition", self.inner.primary.set_projection_definition(definition, rebuild)).await
    }

    pub async fn get_projection_status(
        &self, name: &Identifier, revision: u64, frontier: Option<&StorageCursor>,
    ) -> Result<Option<ProjectionStatus>> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.get_projection_status", self.inner.primary.get_projection_status(name, revision, frontier)).await
    }

    pub async fn list_projection_rows(&self, read: &ProjectionRead) -> Result<ProjectionPage> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.list_projection_rows", self.inner.primary.list_projection_rows(read)).await
    }
    pub async fn delete_projection_history(&self, name: &Identifier, revision: u64, through: &StorageCursor) -> Result<()> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.delete_projection_history", self.inner.primary.delete_projection_history(name, revision, through)).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
