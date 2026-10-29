/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

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

#[derive(Clone, Default)]
struct CleanupCapture(std::sync::Arc<std::sync::OnceLock<harmos_runtime::runtime::BootstrapCleanup>>);

impl CleanupCapture {
    fn capture<A: harmos_runtime::Application>(&self, setup: &mut harmos_runtime::Setup<A>) {
        assert!(self.0.set(setup.settings().startup_cleanup()).is_ok());
    }

    fn get(&self) -> &harmos_runtime::runtime::BootstrapCleanup {
        self.0.get().expect("startup was polled")
    }
}

fn incompatible(error: harmos_runtime::StartupError) -> harmos_runtime::runtime::Incompatible {
    match error {
        harmos_runtime::StartupError::Runtime { source } => *source.downcast().expect("physical startup failure"),
        other => panic!("unexpected startup failure: {other}"),
    }
}

// A Dropped Boot Joins Its Cleanup Before Storage Reports Closed.
async fn closed(storage: &harmos_runtime::storage::StorageManager, cleanup: &harmos_runtime::runtime::BootstrapCleanup) {
    cleanup.wait().await.unwrap();
    let status = storage.status();
    assert!(status.closed && !status.running);
}
