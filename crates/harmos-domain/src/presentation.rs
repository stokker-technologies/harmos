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
use serde::{Deserialize, Serialize};
// Local Imports
use crate::Metadata;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamField {
    pub metadata: Metadata,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
}

/* === Enums === */

/* === Traits === */
pub trait RowMetadata {
    const ROW_METADATA_JSON: &'static str = "[]";

    fn row_metadata() -> Vec<StreamField> {
        serde_json::from_str(Self::ROW_METADATA_JSON).expect("macro-generated row metadata is valid")
    }
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
