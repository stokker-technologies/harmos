/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use serde::{Deserialize, Serialize};

use crate::Metadata;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamField {
    pub metadata: Metadata,

    #[serde(default)]
    pub role: Option<String>,

    #[serde(default)]
    pub unit: Option<String>,
}

pub trait RowMetadata {
    const ROW_METADATA_JSON: &'static str = "[]";

    fn row_metadata() -> Vec<StreamField> {
        serde_json::from_str(Self::ROW_METADATA_JSON).expect("macro-generated row metadata is valid")
    }
}
