/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::{Echoed, PROBE};

#[cfg_attr(not(target_arch = "wasm32"), harmos_domain::operation(harmos_path = ::harmos_domain, id = "probe.fail", response = Echoed, policy = PROBE))]
#[cfg_attr(target_arch = "wasm32", harmos::guest::operation(harmos_path = ::harmos::guest, id = "probe.fail", response = Echoed, policy = PROBE))]
#[derive(Clone, PartialEq)]

pub struct FailOperation {}

#[cfg(target_arch = "wasm32")]
mod runtime {

    use super::{Echoed, FailOperation};
    use harmos::guest::OperationRefusal;

    impl harmos::guest::Operation for FailOperation {
        type Error = OperationRefusal;

        async fn run(
            self, _sdk: harmos::guest::Context<'_, Self>,
        ) -> Result<harmos::guest::operations::Response<Echoed>, OperationRefusal> {
            let result: Result<Echoed, OperationRefusal> = async move { panic!("requested guest trap") }.await;
            result.map(Into::into)
        }
    }
}
