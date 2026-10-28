/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod export;

pub use export::{ExportCompleted, ExportFormat};

use serde::{Deserialize, Serialize};

#[harmos::records]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Records {
    ExportCompleted(ExportCompleted),
}
