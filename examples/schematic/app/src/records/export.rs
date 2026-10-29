/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::fmt;

use serde::{Deserialize, Serialize};

#[harmos::record(id = "export.completed")]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ExportCompleted {
    pub format: ExportFormat,
    pub symbols: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum ExportFormat {
    Svg,
    Netlist,
}

impl fmt::Display for ExportFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Svg => "SVG",
            Self::Netlist => "netlist",
        })
    }
}
