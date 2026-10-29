/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Progress Published By The Native Analysis Sidecar.

#[harmos_domain::stream(id = "schematic.analysis_progress", window = 16)]
#[harmos_domain::message]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AnalysisProgress {
    pub title: String,
    pub checked: u32,
}
