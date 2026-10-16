/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
use crate::backend::storage_settings;

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

/* === Tests === */

fn construct(settings: StorageSettings) -> harmos::Result<StorageManager> {
    let backends = StorageBackends {
        primary: Arc::new(Data::new(history())),
        state: Arc::new(Recovery::new(history())),
        replicas: Vec::new(),
    };
    StorageManager::new(backends, settings)
}

use super::*;

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("producer_timeout_explicit", || {
            let mut settings = storage_settings();
            assert_eq!(settings.file_session_timeout, Duration::from_secs(60));
            settings.file_session_timeout = Duration::ZERO;
            assert!(construct(settings.clone()).is_err());
            settings.file_session_timeout = Duration::MAX;
            assert!(construct(settings.clone()).is_err());
            settings.file_session_timeout = Duration::from_millis(1);
            construct(settings.clone()).unwrap();
        });
    }
    {
        scenario_runtime::check("byte_limits_checked", || {
            let mut settings = storage_settings();
            construct(settings.clone()).unwrap();
            settings.target_batch_bytes =
                NonZeroUsize::new(settings.max_batch_bytes.get() + 1).unwrap();
            assert!(construct(settings.clone()).is_err());
            settings.target_batch_bytes = NonZeroUsize::new(1).unwrap();
            settings.max_batch_bytes =
                NonZeroUsize::new(settings.request.max_pending_write_bytes.get() + 1).unwrap();
            assert!(construct(settings.clone()).is_err());
            settings.max_batch_bytes = settings.request.max_pending_write_bytes;
            construct(settings.clone()).unwrap();
        });
    }
}
