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
use crate::streams::Reading;
use harmos::sidecar::{Refusal, Supervisor};
use minibench_contract::probe::{ReadSample, Sample as Request, Sampled};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Sample {
    resources: Arc<Resources>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos::operation(declaration = ReadSample)]
impl Sample {
    fn new(resources: Arc<Resources>) -> Result<Self, Refusal> {
        Ok(Self { resources })
    }

    async fn run(
        &mut self,
        context: &Supervisor,
        request: &mut Request,
    ) -> Result<Sampled, Refusal> {
        let millivolts = measurement(request.channel)?;
        context
            .publisher::<crate::ProbeBridge>()
            .publish_on::<Reading>(&Reading {
                device: self.resources.device.clone(),
                channel: request.channel,
                millivolts,
            })
            .await
            .map_err(Refusal::of)?;
        Ok(Sampled { millivolts })
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn measurement(channel: u32) -> Result<i32, Refusal> {
    i32::try_from(channel)
        .ok()
        .and_then(|channel| channel.checked_mul(10))
        .and_then(|offset| 3_300_i32.checked_add(offset))
        .ok_or_else(|| Refusal::of("probe channel exceeds measurement range"))
}

/* === Tests === */
