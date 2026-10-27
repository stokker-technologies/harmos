/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Read Operation Contract And Execution.

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

#[harmos_guest::operation(harmos_path = ::harmos_guest,
    resources = [crate::Owner, harmos_guest::kit::Context],
    operations = [ReadOperation], id = "fixture.read", response = Value, policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_millis(500),
        retries: RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: Jitter::None,
            },
        },
    })]
#[derive(Clone, PartialEq)]

pub struct ReadOperation {
    pub value: String,
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
    use super::super::Stage;
    use super::{ReadOperation, Value};

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl harmos_guest::Operation for ReadOperation {
        type Error = std::io::Error;

        async fn run(mut self, ctx: harmos_guest::Context<'_, Self>) -> Result<harmos_guest::Response<Value>, Self::Error> {
            let owner = ctx.resource::<crate::Owner>();
            let carrier = ctx.resource::<harmos_guest::kit::Context>();
            owner.ran.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            // Call Metadata And Explicit Child Overlays
            if self.value == "context-leaf" {
                let stage = ctx.call().value::<Stage>().map_err(std::io::Error::other)?.expect("the caller set a stage");
                return Ok(Value { value: stage.id }.into());
            }
            if self.value == "context-parent" {
                let parent = ctx.call().value::<Stage>().map_err(std::io::Error::other)?.expect("the caller set a stage");
                let caller = carrier.with_value(Stage { id: "child".into() }).map_err(std::io::Error::other)?;
                let child = harmos_guest::Host::call::<ReadOperation>(&caller, ReadOperation { value: "context-leaf".into() })
                    .await
                    .map_err(std::io::Error::other)?;
                return Ok(Value {
                    value: format!("{}:{}", parent.id, child.value),
                }
                .into());
            }
            if self.value == "leaf" {
                return Ok(Value { value: "leaf:1".into() }.into());
            }

            // Retry State Belongs To The Declared Resource, Never The Request
            let attempt = {
                let mut attempts = owner.attempts.lock().map_err(|error| std::io::Error::other(error.to_string()))?;
                let attempt = attempts.entry(self.value.clone()).or_default();
                *attempt += 1;
                *attempt
            };
            if self.value == "parent" {
                let saved = owner.saved_attempts.lock().map_err(|error| std::io::Error::other(error.to_string()))?.remove(&self.value);
                if let Some(saved) = saved
                    && harmos_guest::Host::call::<ReadOperation>(&saved, ReadOperation { value: "leaf".into() }).await.is_ok()
                {
                    return Err(std::io::Error::other("stale context was accepted"));
                }
                let child = ctx.operation(ReadOperation { value: "leaf".into() }).await.map_err(std::io::Error::other)?;
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
                    .insert(self.value.clone(), carrier.as_ref().clone());
            }
            if attempt == 1 {
                // The Next Attempt Must Receive The Original Request Clone
                self.value.push_str(":mutated");
                return Err(std::io::Error::other("first transient attempt"));
            }
            Ok(Value {
                value: format!("{}:{attempt}", self.value),
            }
            .into())
        }

        fn classify(_: &Self::Error) -> harmos_guest::operations::FailureKind {
            harmos_guest::operations::FailureKind::Transient
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}
