/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Test-Only State Journal Access

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Journal(StateLog);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Journal {
    pub fn open(
        root: &Path, history: StoreHistory, settings: StorageStateSettings, opener: JournalOpener, check: &mut impl FnMut() -> Result<()>,
    ) -> Result<Self> {
        StateLog::open(root, history, settings, opener, check).map(Self)
    }

    pub fn position(&self) -> RecoveryPosition {
        self.0.position()
    }

    pub fn append(&mut self, entries: &[RecoveryEntry]) -> Result<()> {
        self.0.append(entries)
    }

    pub fn page(&mut self, after: &RecoveryPosition, through: &RecoveryPosition, limits: ReadLimits) -> Result<RecoveryPage> {
        self.0.page(after, through, limits)
    }

    pub fn checkpoint(&mut self, snapshot: &RecoverySnapshot) -> Result<()> {
        self.0.checkpoint(snapshot)
    }

    pub fn snapshot(&self, maximum: usize) -> Result<Option<RecoverySnapshot>> {
        self.0.snapshot(maximum)
    }

    pub fn offset_bytes(&self) -> Result<u64> {
        self.0.offsets.metadata().map(|metadata| metadata.len()).map_err(super::io)
    }

    pub fn decoded(&self) -> usize {
        self.0.decoded
    }

    pub fn reset_decoded(&mut self) {
        self.0.decoded = 0;
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
