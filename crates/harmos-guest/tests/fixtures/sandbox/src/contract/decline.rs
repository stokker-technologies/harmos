/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::{Echoed, PROBE};

#[cfg_attr(not(target_arch = "wasm32"), harmos_domain::operation(harmos_path = ::harmos_domain, id = "probe.decline", response = Echoed, policy = PROBE))]
#[cfg_attr(target_arch = "wasm32", harmos::guest::operation(harmos_path = ::harmos::guest, id = "probe.decline", response = Echoed, policy = PROBE))]
#[derive(Clone, PartialEq)]

pub struct DeclineOperation {}

#[cfg(target_arch = "wasm32")]
mod runtime {

    use super::{DeclineOperation, Echoed};
    use harmos::guest::OperationRefusal;

    impl harmos::guest::Operation for DeclineOperation {
        type Error = OperationRefusal;

        async fn run(
            self, _sdk: harmos::guest::Context<'_, Self>,
        ) -> Result<harmos::guest::operations::Response<Echoed>, OperationRefusal> {
            let result: Result<Echoed, OperationRefusal> = async move { Err(OperationRefusal::new(7, "the probe declines")) }.await;
            result.map(Into::into)
        }
    }
}
