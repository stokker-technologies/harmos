/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::Resources;
use harmos::sidecar::Refusal;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[derive(Clone, PartialEq, serde::Deserialize)]
pub struct Recalibration {
    pub offset: f64,
}

#[harmos::listener(id = "standalone.recalibrated-listener", record = "standalone.recalibrated", version = 1, row = Recalibration, callback = Heard)]
pub struct Recalibrated;

pub struct Heard {
    resources: Arc<Resources>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos::operation(
    id = "standalone-bridge.recalibrated",
    version = 1,
    visibility = harmos::operations::Visibility::Shared,
    policy = harmos::operations::OperationPolicy {
        timeout: std::time::Duration::from_secs(30),
        attempt_timeout: std::time::Duration::from_secs(5),
        retries: harmos::operations::RetryPolicy::Never,
        restart: harmos::operations::RestartPolicy::Never,
    }
)]
impl Heard {
    pub async fn initialize(resources: Arc<Resources>, _: &harmos::sidecar::Supervisor) -> Self {
        Self { resources }
    }

    async fn run(
        &mut self,
        _: &harmos::sidecar::Supervisor,
        delivery: &harmos::listeners::Delivery<Recalibration>,
    ) -> Result<(), Refusal> {
        eprintln!(
            "{} was recalibrated by {}",
            self.resources.instrument,
            delivery.row().offset
        );
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
