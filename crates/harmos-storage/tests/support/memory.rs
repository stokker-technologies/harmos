/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Allocate Read Admission Only For Stories That Consume Backend Reads.

/* === Modules === */
/* === Re-Exports === */
/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
// Third Party Imports
// Local Imports
use super::backend::{Data, Recovery, history, storage_settings};

/* === Constants === */
/* === Macros === */
/* === Type Aliases === */
/* === Structs === */
/* === Enums === */
/* === Traits === */
/* === Implementations === */
/* === Public Functions === */

pub async fn read() -> harmos_storage::arrow::RetainedMemory {
    let manager = harmos_storage::StorageManager::<()>::new(
        harmos_storage::StorageBackends {
            primary: std::sync::Arc::new(Data::<()>::new(history())),
            state: std::sync::Arc::new(Recovery::new(history())),
        },
        storage_settings(),
    )
    .unwrap();
    manager.read_memory(NonZeroUsize::new(32 * 1024).unwrap()).await.unwrap()
}

/* === Private Functions === */
/* === Tests === */
