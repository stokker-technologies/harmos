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
    pub fn capabilities(&self) -> StorageCapabilities {
        self.inner.primary.capabilities()
    }
    pub async fn register_index(
        &self, definition: &IndexDefinition, rebuild: bool, work: ReadLimits, cancel: &Cancellation,
    ) -> Result<u64> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.register_index", self.inner.primary.register_index(definition, rebuild, work, cancel)).await
    }
    pub async fn register_projection(&self, definition: &ProjectionDefinition, rebuild: bool) -> Result<u64> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.register_projection", self.inner.primary.register_projection(definition, rebuild)).await
    }
    pub async fn projection_status(&self, name: &Identifier, revision: u64, frontier: &StorageCursor) -> Result<Option<ProjectionStatus>> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.projection_status", self.inner.primary.projection_status(name, revision, frontier)).await
    }
    pub async fn projection_frontier(&self, name: &Identifier, revision: u64) -> Result<Option<StorageCursor>> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.projection_frontier", self.inner.primary.projection_frontier(name, revision)).await
    }
    pub async fn read_projection(&self, read: &ProjectionRead) -> Result<ProjectionPage> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.read_projection", self.inner.primary.read_projection(read)).await
    }
    pub async fn retire_projection(&self, name: &Identifier, revision: u64, through: &StorageCursor) -> Result<()> {
        let _lifecycle = self.inner.lifecycle.read().await;
        crate::capacity::admitting(&self.inner.state.lock().unwrap(), false)?;
        self.inner.bounded("primary.retire_projection", self.inner.primary.retire_projection(name, revision, through)).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
