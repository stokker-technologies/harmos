/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

impl Manager {
    pub(crate) fn close_admission(&self) {
        self.close_entries(true);
    }

    pub(crate) fn close(&self) {
        self.close_entries(false);
    }

    pub(super) fn close_entries(&self, _retain_shutdown: bool) {
        let mut state = self.registry.state.lock().expect("operation registry lock is healthy");
        if _retain_shutdown && state.closing {
            return;
        }
        state.closing = true;
        let retired = std::mem::take(&mut state.cache);
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        let staging: Vec<_> = state.staging.iter().filter_map(std::sync::Weak::upgrade).collect();
        for entry in state.entries.values().flat_map(|providers| providers.values()) {
            let _pending = entry.pending.lock().expect("operation settlement lock is healthy");
            if _retain_shutdown && entry.retirement_control() {
                continue;
            }
            if _retain_shutdown && entry.active.load(Ordering::Acquire) {
                entry.retirement.store(Retirement::Runtime as u8, Ordering::Release);
            }
            entry.active.store(false, Ordering::Release);
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            let ordinary = entry.sdk.is_none();
            #[cfg(not(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar")))]
            let ordinary = true;
            if _retain_shutdown && ordinary {
                for control in _pending.values() {
                    control.cancel();
                }
            }
        }
        drop(state);
        drop(retired);
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        for scope in staging {
            scope.close_staging();
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.close();
        self.instances.close();
        let entries: Vec<_> = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy")
            .entries
            .values()
            .flat_map(|providers| providers.values())
            .cloned()
            .collect();
        for entry in entries {
            settle(&entry).await?;
        }
        self.instances.settle().await?;
        let retired = std::mem::take(&mut self.registry.state.lock().expect("operation registry lock is healthy").entries);
        drop(retired);
        Ok(())
    }
}
