/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod admission;
mod boot;
mod capture;
mod composition;
mod files;
mod managed;
mod reconcile;
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
async fn closed(
    storage: &harmos_runtime::storage::StorageManager,
    cleanup: &harmos_runtime::runtime::BootstrapCleanup,
) {
    cleanup.wait().await.unwrap();
    let status = storage.status();
    assert!(status.closed && !status.running);
}

/* === Tests === */
