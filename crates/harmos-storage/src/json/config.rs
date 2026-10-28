/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Explicit JSON Storage Paths And Entry Bounds.

use std::{num::NonZeroUsize, path::PathBuf};

#[derive(Clone, Debug)]
pub struct JsonStorageSettings {
    pub root: PathBuf,
    pub max_entry_bytes: NonZeroUsize,
}
