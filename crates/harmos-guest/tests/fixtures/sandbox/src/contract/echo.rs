/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

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

#[derive(::harmos_guest::Message, Debug, Default)]
#[harmos_path(::harmos_guest)]
#[derive(::harmos_guest::Request)]
#[request(response = Echoed)]
#[derive(Clone, PartialEq)]

pub struct EchoOperation {
    pub say: String,
}

pub static ECHO_OPERATION_DESCRIPTION: ::harmos_guest::operations::WireDeclaration =
    ::harmos_guest::operations::WireDeclaration::request::<EchoOperation>("probe.echo", PROBE)
        .visibility(::harmos_guest::operations::Visibility::Public);

static ECHO_OPERATION_CALLEES: [::harmos_guest::operations::WireDeclaration; 2] =
    [super::provider::PROVIDER_ECHO_OPERATION_DESCRIPTION.copied(), harmos_guest::builtins::streams::PUBLISH_DESCRIPTION.copied()];
pub const ECHO_OPERATION_OPERATION: ::harmos_guest::operations::OperationSpec<EchoOperation> =
    ::harmos_guest::operations::OperationSpec::<EchoOperation>::new(&ECHO_OPERATION_DESCRIPTION).calls(&ECHO_OPERATION_CALLEES);

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
    use crate::contract::{ECHO_OPERATION_OPERATION, PROVIDER_ECHO_OPERATION_OPERATION};
    use harmos_guest::{OperationHost, OperationRefusal};

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    pub async fn echo_operation_handler(
        request: EchoOperation, sdk: harmos_guest::operations::FunctionContext<EchoOperation>,
    ) -> Result<harmos_guest::operations::Response<Echoed>, OperationRefusal> {
        let owner = sdk.resource::<crate::Owner>().expect("declared resource is registered");
        let carrier = sdk.resource::<harmos_guest::kit::Context>().expect("declared resource is registered");
        let context = carrier.as_ref();
        let result: Result<Echoed, OperationRefusal> = async move {
            // Calls Through the Saved Initialization Context
            if let Some(value) = request.say.strip_prefix("saved-publish:") {
                let saved = owner
                    .initialized_context
                    .lock()
                    .map_err(OperationRefusal::of)?
                    .clone()
                    .ok_or_else(|| OperationRefusal::of("plugin context is absent"))?;
                crate::progress(&saved, value).await.map_err(OperationRefusal::of)?;
                return Ok(Echoed { said: format!("published {value}") });
            }
            if let Some(value) = request.say.strip_prefix("saved-dependency:") {
                let saved = owner
                    .initialized_context
                    .lock()
                    .map_err(OperationRefusal::of)?
                    .clone()
                    .ok_or_else(|| OperationRefusal::of("plugin context is absent"))?;
                return saved
                    .operation::<super::super::ProviderEchoOperation>(ProviderEchoOperation { say: value.into() })
                    .await
                    .map(harmos_guest::operations::Response::into_owned)
                    .map_err(OperationRefusal::of);
            }

            // Calls Through the Current Context
            if let Some(say) = request.say.strip_prefix("dependency:") {
                let invocation = sdk.operation(PROVIDER_ECHO_OPERATION_OPERATION, ProviderEchoOperation { say: say.to_owned() });
                let response = match sdk.call().value::<super::super::RetryChoice>().map_err(OperationRefusal::of)? {
                    Some(settings) => invocation.retry(settings.policy()).await,
                    None => invocation.await,
                };
                return response.map(harmos_guest::operations::Response::into_owned).map_err(OperationRefusal::from);
            }
            if let Some(value) = request.say.strip_prefix("publish:") {
                crate::progress(context, value).await.map_err(OperationRefusal::from)?;
            }
            if request.say == "publish-many" {
                for index in 0..8 {
                    crate::progress(context, &index.to_string()).await.map_err(OperationRefusal::from)?;
                }
            }

            // Memory the Sandbox Must Refuse
            if request.say == "grow" {
                std::hint::black_box(vec![1u8; 128 * 1024 * 1024]);
            }
            Ok(Echoed { said: format!("heard {}", request.say) })
        }
        .await;
        result.map(Into::into)
    }

    pub fn echo_operation_operation()
    -> ::core::result::Result<harmos_guest::operations::OperationFactory, harmos_guest::operations::ServingError> {
        harmos_guest::operations::OperationFactory::function(
            ECHO_OPERATION_OPERATION,
            const { &[harmos_guest::Resource::of::<crate::Owner>(), harmos_guest::Resource::of::<harmos_guest::kit::Context>()] },
            echo_operation_handler,
            |_| harmos_guest::operations::FailureKind::Permanent,
        )
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

#[cfg(target_arch = "wasm32")]
pub use runtime::echo_operation_operation;
