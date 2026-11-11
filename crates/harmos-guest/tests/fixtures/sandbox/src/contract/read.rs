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
use super::{Backoff, Duration, Jitter, OperationPolicy, RetryPolicy, Value};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(::harmos_guest::Message, Debug, Default)]
#[harmos_path(::harmos_guest)]
#[derive(::harmos_guest::Request)]
#[request(response = Value)]
#[derive(Clone, PartialEq)]

pub struct ReadOperation {
    pub value: String,
}

pub static READ_OPERATION_DESCRIPTION: ::harmos_guest::operations::WireDeclaration =
    ::harmos_guest::operations::WireDeclaration::request::<ReadOperation>(
        "fixture.read",
        OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_millis(500),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::OnTransient {
                max_retries: 1,
                backoff: Backoff::Fixed {
                    delay: Duration::from_millis(1),
                    jitter: Jitter::None,
                },
            },
        },
    );

static READ_OPERATION_CALLEES: [::harmos_guest::operations::WireDeclaration; 1] = [READ_OPERATION_DESCRIPTION.copied()];
pub const READ_OPERATION_OPERATION: ::harmos_guest::operations::OperationSpec<ReadOperation> =
    ::harmos_guest::operations::OperationSpec::<ReadOperation>::new(&READ_OPERATION_DESCRIPTION).calls(&READ_OPERATION_CALLEES);

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
    use super::super::Stage;
    use super::{ReadOperation, Value};
    use crate::contract::READ_OPERATION_OPERATION;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    pub async fn read_operation_handler(
        mut request: ReadOperation, ctx: harmos_guest::operations::FunctionContext<ReadOperation>,
    ) -> Result<harmos_guest::Response<Value>, std::io::Error> {
        let owner = ctx.resource::<crate::Owner>().expect("declared resource is registered");
        let carrier = ctx.resource::<harmos_guest::kit::Context>().expect("declared resource is registered");
        owner.ran.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Call Metadata and Explicit Child Overlays
        if request.value == "context-leaf" {
            let stage = ctx.call().value::<Stage>().map_err(std::io::Error::other)?.expect("the caller set a stage");
            return Ok(Value { value: stage.id }.into());
        }
        if request.value == "context-parent" {
            let parent = ctx.call().value::<Stage>().map_err(std::io::Error::other)?.expect("the caller set a stage");
            let caller = carrier.with_value(Stage { id: "child".into() }).map_err(std::io::Error::other)?;
            let child = harmos_guest::OperationHost::operation::<ReadOperation>(&caller, ReadOperation { value: "context-leaf".into() })
                .await
                .map_err(std::io::Error::other)?;
            return Ok(Value {
                value: format!("{}:{}", parent.id, child.value),
            }
            .into());
        }
        if request.value == "leaf" {
            return Ok(Value { value: "leaf:1".into() }.into());
        }

        // Retry State Belongs to the Declared Resource, Never the Request
        let attempt = {
            let mut attempts = owner.attempts.lock().map_err(|error| std::io::Error::other(error.to_string()))?;
            let attempt = attempts.entry(request.value.clone()).or_default();
            *attempt += 1;
            *attempt
        };
        if request.value == "parent" {
            let saved = owner.saved_attempts.lock().map_err(|error| std::io::Error::other(error.to_string()))?.remove(&request.value);
            if let Some(saved) = saved
                && harmos_guest::OperationHost::operation::<ReadOperation>(&saved, ReadOperation { value: "leaf".into() }).await.is_ok()
            {
                return Err(std::io::Error::other("stale context was accepted"));
            }
            let child =
                ctx.operation(READ_OPERATION_OPERATION, ReadOperation { value: "leaf".into() }).await.map_err(std::io::Error::other)?;
            if attempt > 1 {
                return Ok(Value {
                    value: format!("parent:{attempt}:stale:{}", child.value),
                }
                .into());
            }
            owner
                .saved_attempts
                .lock()
                .map_err(|error| std::io::Error::other(error.to_string()))?
                .insert(request.value.clone(), carrier.as_ref().clone());
        }
        if attempt == 1 {
            // The Next Attempt Must Receive the Original Request Clone
            request.value.push_str(":mutated");
            return Err(std::io::Error::other("first transient attempt"));
        }
        Ok(Value {
            value: format!("{}:{attempt}", request.value),
        }
        .into())
    }

    fn read_operation_classification(_: &std::io::Error) -> harmos_guest::operations::FailureKind {
        harmos_guest::operations::FailureKind::Transient
    }

    pub fn read_operation_operation()
    -> ::core::result::Result<harmos_guest::operations::OperationFactory, harmos_guest::operations::ServingError> {
        harmos_guest::operations::OperationFactory::function(
            READ_OPERATION_OPERATION,
            const { &[harmos_guest::Resource::of::<crate::Owner>(), harmos_guest::Resource::of::<harmos_guest::kit::Context>()] },
            read_operation_handler,
            read_operation_classification,
        )
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

#[cfg(target_arch = "wasm32")]
pub use runtime::read_operation_operation;
