/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Structs === */

/// Cumulative measured persistence work, not scheduling or admission policy.
/// Commit bytes report checked encoded upper bounds, not physical device traffic.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct PersistenceWork {
    pub cadence_wait: StageWork,
    pub ordered_wait: StageWork,
    pub ordered_wait_small: StageWork,
    pub ordered_wait_large: StageWork,
    pub batch_build: StageWork,
    pub backend: StageWork,
    pub publication: StageWork,
    pub committed_pieces: u64,
    pub committed_rows: u64,
    pub committed_bytes: u64,
    pub maximum_commit_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct StageWork {
    pub count: u64,
    pub total_ns: u64,
    pub maximum_ns: u64,
}

/* === Implementations === */

impl StageWork {
    pub(super) fn record(&mut self, elapsed: std::time::Duration) {
        let ns = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.count = self.count.saturating_add(1);
        self.total_ns = self.total_ns.saturating_add(ns);
        self.maximum_ns = self.maximum_ns.max(ns);
    }
}
