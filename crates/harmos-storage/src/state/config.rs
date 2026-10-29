/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// State Backend Resource Limits

use std::num::NonZeroUsize;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageStateSettings {
    pub max_entry_bytes: NonZeroUsize,

    pub max_checkpoint_bytes: NonZeroUsize,
}
