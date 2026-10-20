/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "state/admission.rs"]
mod admission;
#[path = "state/boot.rs"]
mod boot;
#[path = "state/capture.rs"]
mod capture;
#[path = "state/composition.rs"]
mod composition;
#[path = "state/files.rs"]
mod files;
#[path = "state/managed.rs"]
mod managed;
#[path = "state/reconcile.rs"]
mod reconcile;
#[path = "state/snapshots.rs"]
mod snapshots;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

// A Dropped Boot Joins Its Cleanup Before Storage Reports Closed.
async fn closed(storage: &harmos_runtime::storage::StorageManager, cleanup: &harmos_runtime::runtime::BootstrapCleanup) {
    cleanup.wait().await.unwrap();
    let status = storage.status();
    assert!(status.closed && !status.running);
}

/* === Tests === */
