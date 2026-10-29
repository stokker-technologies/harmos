/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::{ReadOperation, Value};

#[cfg_attr(not(target_arch = "wasm32"), harmos_domain::operation(harmos_path = ::harmos_domain,
    resources = [crate::Owner], id = "fixture.counts", response = Value, policy = ReadOperation::POLICY))]
#[cfg_attr(target_arch = "wasm32", harmos::guest::operation(harmos_path = ::harmos::guest,
    resources = [crate::Owner], id = "fixture.counts", response = Value, policy = ReadOperation::POLICY))]
#[derive(Clone, PartialEq)]

pub struct CountsOperation {
    pub value: String,
}

#[cfg(target_arch = "wasm32")]
mod runtime {

    use super::{CountsOperation, Value};

    impl harmos::guest::Operation for CountsOperation {
        type Error = std::io::Error;

        async fn run(self, sdk: harmos::guest::Context<'_, Self>) -> Result<harmos::guest::operations::Response<Value>, std::io::Error> {
            let owner = sdk.resource::<crate::Owner>();
            Ok(Value {
                value: owner.ran.load(std::sync::atomic::Ordering::SeqCst).to_string(),
            }
            .into())
        }
    }
}
