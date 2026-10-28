/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub struct AnalysisResources {
    pub(crate) analysis: tokio::sync::Semaphore,
}

impl Default for AnalysisResources {
    fn default() -> Self {
        Self { analysis: tokio::sync::Semaphore::new(1) }
    }
}
