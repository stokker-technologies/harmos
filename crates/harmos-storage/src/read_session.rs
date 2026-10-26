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

// Local Imports
use crate::capacity::{Held, Pool, Refusal, Reservation};
use crate::manager::ENTRY_OVERHEAD;
use crate::{MemorySize, ReadBatch, Result, StorageManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Shared admission context for one composed read operation.
///
/// A session does not freeze cursors or create a transaction snapshot. Its readers,
/// point reads and collections share memory dependencies. Grouped reader creation
/// refuses saturated active slots instead of waiting on another reader in the group.
/// Returned owners remain valid and charged after the session drops.
pub struct ReadSession<C = ()> {
    pub(crate) manager: StorageManager<C>,
    pub(crate) held: Held,
    pub(crate) queued: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> Clone for ReadSession<C> {
    fn clone(&self) -> Self {
        Self { manager: self.manager.clone(), held: self.held.clone(), queued: self.queued }
    }
}

impl<C> StorageManager<C> {
    pub fn read_session(&self) -> ReadSession<C> {
        ReadSession { manager: self.clone(), held: Held::default(), queued: false }
    }
}

impl<C> ReadSession<C> {
    pub(crate) fn standalone(manager: &StorageManager<C>) -> Self {
        Self { manager: manager.clone(), held: Held::default(), queued: true }
    }

    /// Admits mutable metadata capacity in this session without waiting.
    ///
    /// The claim includes a private owner allowance. Keep metadata and its claim
    /// under the same owner, and use `try_grow_by` before allocating incremental
    /// keys, nodes, replacement buffers or cursor metadata. Failed growth leaves
    /// existing data and admission unchanged.
    pub fn try_memory(&self, payload_bytes: std::num::NonZeroUsize) -> Result<crate::arrow::RetainedMemory> {
        let total =
            payload_bytes.get().checked_add(ENTRY_OVERHEAD).ok_or(crate::Error::ResourceExhausted { resource: "session read memory" })?;
        Reservation::try_new(&self.manager.inner, Pool::Read, &self.held, total).map(Reservation::retain).map_err(Refusal::into_error)
    }

    pub async fn read_file(&self, request: harmos_domain::operations::storage::ReadFile) -> Result<crate::FileReader> {
        self.manager.read_file_with_held(request, self.held.clone()).await
    }

    /// Creates an empty retained collection for this session's projected pages.
    ///
    /// Grow the outer Vec through `ReadBatch::try_map`, admitting the full temporary
    /// replacement backing before allocating it. Child pages retain their own claims.
    /// Keep dependent reads in this session; unrelated groups do not share its Held.
    pub fn collection<U: MemorySize>(&self) -> Result<ReadBatch<Vec<ReadBatch<U>>>> {
        let reservation = Reservation::try_new(&self.manager.inner, Pool::Read, &self.held, ENTRY_OVERHEAD).map_err(Refusal::into_error)?;
        Ok(ReadBatch::new(Vec::new(), reservation))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
