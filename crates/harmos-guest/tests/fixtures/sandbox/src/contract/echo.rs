/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Echo Operation Contract And Execution.

/* === --- === Definition Module === --- === */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{Echoed, PROBE};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[cfg_attr(not(target_arch = "wasm32"), harmos_domain::operation(
    harmos_path = ::harmos_domain,
    resources = [crate::Owner, harmos_guest::kit::Context],
    operations = [super::ProviderEchoOperation, harmos_guest::builtins::streams::Publish],
    id = "probe.echo",
    response = Echoed,
    policy = PROBE,
    visibility = Shared,
))]
#[cfg_attr(target_arch = "wasm32", harmos::guest::operation(
    harmos_path = ::harmos::guest,
    resources = [crate::Owner, harmos::guest::kit::Context],
    operations = [super::ProviderEchoOperation, harmos::guest::builtins::streams::Publish],
    id = "probe.echo",
    response = Echoed,
    policy = PROBE,
    visibility = Shared,
))]
#[derive(Clone, PartialEq)]

pub struct EchoOperation {
    pub say: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

/* === --- === Runtime Module === --- === */

#[cfg(target_arch = "wasm32")]
mod runtime {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports

    // Local Imports
    use super::super::ProviderEchoOperation;
    use super::{EchoOperation, Echoed};
    use harmos::guest::{Host, OperationRefusal};

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl harmos::guest::Operation for EchoOperation {
        type Error = OperationRefusal;

        async fn run(self, sdk: harmos::guest::Context<'_, Self>) -> Result<harmos::guest::operations::Response<Echoed>, OperationRefusal> {
            let owner = sdk.resource::<crate::Owner>();
            let carrier = sdk.resource::<harmos::guest::kit::Context>();
            let context = carrier.as_ref();
            let result: Result<Echoed, OperationRefusal> = async move {
                // Calls Through The Saved Initialization Context
                if let Some(value) = self.say.strip_prefix("saved-publish:") {
                    let saved = owner
                        .initialized_context
                        .lock()
                        .map_err(OperationRefusal::of)?
                        .clone()
                        .ok_or_else(|| OperationRefusal::of("plugin context is absent"))?;
                    crate::progress(&saved, value).await.map_err(OperationRefusal::of)?;
                    return Ok(Echoed { said: format!("published {value}") });
                }
                if let Some(value) = self.say.strip_prefix("saved-dependency:") {
                    let saved = owner
                        .initialized_context
                        .lock()
                        .map_err(OperationRefusal::of)?
                        .clone()
                        .ok_or_else(|| OperationRefusal::of("plugin context is absent"))?;
                    return saved
                        .call::<super::super::ProviderEchoOperation>(ProviderEchoOperation { say: value.into() })
                        .await
                        .map(harmos::guest::operations::Response::into_owned)
                        .map_err(OperationRefusal::of);
                }

                // Calls Through The Current Context
                if let Some(say) = self.say.strip_prefix("dependency:") {
                    return sdk
                        .operation(ProviderEchoOperation { say: say.to_owned() })
                        .await
                        .map(harmos::guest::operations::Response::into_owned)
                        .map_err(OperationRefusal::of);
                }
                if let Some(value) = self.say.strip_prefix("publish:") {
                    crate::progress(context, value).await.map_err(OperationRefusal::from)?;
                }
                if self.say == "publish-many" {
                    for index in 0..8 {
                        crate::progress(context, &index.to_string()).await.map_err(OperationRefusal::from)?;
                    }
                }

                // Memory The Sandbox Must Refuse
                if self.say == "grow" {
                    std::hint::black_box(vec![1u8; 128 * 1024 * 1024]);
                }
                Ok(Echoed { said: format!("heard {}", self.say) })
            }
            .await;
            result.map(Into::into)
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}
