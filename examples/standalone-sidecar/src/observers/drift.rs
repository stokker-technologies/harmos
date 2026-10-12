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

// Third Party Imports

// Local Imports
use crate::Resources;
use harmos::sidecar::{ObserverContext, PublishError, Publisher};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::stream(
    description = "One reading of how far the instrument has drifted.",
    id = "drift",
    window = 64
)]
#[harmos::message]
#[derive(Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct DriftRow {
    pub instrument: String,

    pub elapsed_s: f64,

    pub drift: f64,
}

#[derive(Default)]
pub struct Drift {
    accumulated: f64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos::observer(
    description = "Publishes the instrument's accumulated drift while anything leases it.",
    interval_ms = 250,
    streams(DriftRow),
    activation = "demand"
)]
impl Drift {
    async fn observe(
        &mut self,
        ctx: &ObserverContext<'_, Resources, Publisher<crate::StandaloneBridge>>,
    ) -> Result<(), PublishError> {
        self.accumulated += 0.5;

        ctx.publish_on::<DriftRow>(&DriftRow {
            instrument: ctx.resources().instrument.clone(),
            elapsed_s: ctx.tick().elapsed().as_secs_f64(),
            drift: self.accumulated,
        })
        .await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
