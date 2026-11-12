/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

#[tokio::test]
async fn snapshots_require_storage() {
    assert!(
        crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
            setup.settings().managed_storage(harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: Some(policy(100)),
                journal_tail: 1024,
            });
        })
        .await
        .is_err()
    );
}
