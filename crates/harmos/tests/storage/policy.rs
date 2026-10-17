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
    };
    StorageManager::new(backends, settings)
}

use super::*;

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("response_fits_read_budget", || {
            let mut settings = storage_settings();
            construct(settings.clone()).unwrap();
            settings.response.batch_bytes =
                NonZeroUsize::new(settings.budget.read_memory_bytes.get() + 1).unwrap();
            assert!(construct(settings.clone()).is_err());
            settings.response.batch_bytes = settings.budget.read_memory_bytes;
            assert!(construct(settings.clone()).is_err());
            settings.response.batch_bytes = NonZeroUsize::new(settings.budget.read_memory_bytes.get() / 2).unwrap();
            construct(settings).unwrap();
        });
    }
    {
        scenario_runtime::check("query_counts_bounded", || {
            let mut settings = storage_settings();
            settings.request.max_active_queries =
                NonZeroUsize::new(tokio::sync::Semaphore::MAX_PERMITS + 1).unwrap();
            assert!(construct(settings.clone()).is_err());
            settings.request.max_active_queries = NonZeroUsize::MIN;
            settings.request.max_pending_queries = NonZeroUsize::MIN;
            construct(settings).unwrap();
        });
    }
}
