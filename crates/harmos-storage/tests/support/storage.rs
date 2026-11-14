/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "settings.rs"]
mod settings;

/* === Re-Exports === */

pub use settings::storage_settings;
pub use harmos_storage::memory::Backend;
pub use harmos_storage::memory::RecoveryBackend;

/* === Imports === */

use harmos_storage::{Identifier, StoreHistory};

/* === Public Functions === */

pub fn history() -> StoreHistory {
    StoreHistory {
        store: Identifier::new("test-store").unwrap(),
        epoch: Identifier::new("test-epoch").unwrap(),
    }
}
