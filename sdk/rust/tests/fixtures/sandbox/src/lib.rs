/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// WebAssembly Component Adapter

#![allow(unsafe_code, reason = "The Author Supplies One Named Component Registration Symbol")]

/* === Modules === */

pub mod contract;

/* === Re-Exports === */

/* === Imports === */

use contract::*;
use harmos::wasm::Registration;
use harmos::{Classification, Context, Error, Operation, OperationPolicy, OperationRequest, Plugin, Result, Stream, Visibility};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Owner {
    initialized: Mutex<Option<Context>>,
    configuration: Mutex<String>,
    attempts: Mutex<HashMap<String, usize>>,
    saved: Mutex<HashMap<String, Context>>,
    ran: AtomicUsize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

// One Author-Supplied Registration Function; the SDK Supplies Component Exports.
#[unsafe(no_mangle)]
pub fn harmos_register() -> Registration {
    let owner = Arc::new(Owner::default());
    let echo_owner = Arc::clone(&owner);
    let read_owner = Arc::clone(&owner);
    let counts_owner = Arc::clone(&owner);
    let finish_owner = Arc::clone(&owner);
    let initialization_owner = Arc::clone(&owner);
    let shutdown_owner = Arc::clone(&owner);
    let cleanup_owner = Arc::clone(&owner);
    let ordinary = policy(1, 0);
    let retried = OperationPolicy {
        attempt_timeout: Some(harmos::encoding::Duration { seconds: 0, nanos: 500_000_000 }),
        ..policy(2, 1)
    };
    let finish = OperationPolicy {
        timeout: Some(harmos::encoding::Duration { seconds: 5, nanos: 0 }),
        attempt_timeout: Some(harmos::encoding::Duration { seconds: 5, nanos: 0 }),
        cancellation_timeout: Some(harmos::encoding::Duration { seconds: 0, nanos: 100_000_000 }),
        ..ordinary
    };
    let wait = OperationPolicy {
        timeout: Some(harmos::encoding::Duration { seconds: 0, nanos: 5_000_000 }),
        attempt_timeout: Some(harmos::encoding::Duration { seconds: 0, nanos: 5_000_000 }),
        ..ordinary
    };
    let spin = OperationPolicy {
        timeout: Some(harmos::encoding::Duration { seconds: 600, nanos: 0 }),
        attempt_timeout: Some(harmos::encoding::Duration { seconds: 600, nanos: 0 }),
        ..ordinary
    };
    let plugin = Plugin::new("sandbox_guest", env!("CARGO_PKG_VERSION"))
        .operation(
            Operation::new(EchoOperation::ID, async move |context: &Context, request: EchoOperation| {
                echo(&echo_owner, context, request).await
            })
            .visibility(Visibility::Public)
            .policy(ordinary),
        )
        .operation(
            Operation::new(ReadOperation::ID, async move |context: &Context, request: ReadOperation| {
                read(&read_owner, context, request).await
            })
            .visibility(Visibility::Public)
            .replay_safe()
            .policy(retried),
        )
        .operation(
            Operation::new(CountsOperation::ID, async move |_: &Context, _: CountsOperation| {
                Ok(Value {
                    value: counts_owner.ran.load(Ordering::SeqCst).to_string(),
                })
            })
            .visibility(Visibility::Public)
            .policy(ordinary),
        )
        .operation(
            Operation::new(DeclineOperation::ID, async |_: &Context, _: DeclineOperation| -> Result<Echoed> {
                Err(Error::failure(Classification::Permanent, "7", "the probe declines", "Choose Another Probe"))
            })
            .visibility(Visibility::Public)
            .policy(ordinary),
        )
        .operation(
            Operation::new(FailOperation::ID, async |_: &Context, _: FailOperation| -> Result<Echoed> { panic!("requested guest trap") })
                .visibility(Visibility::Public)
                .policy(ordinary),
        )
        .operation(
            Operation::new(SpinOperation::ID, async |_: &Context, _: SpinOperation| -> Result<Echoed> {
                loop {
                    std::hint::black_box(1);
                }
            })
            .visibility(Visibility::Public)
            .policy(spin),
        )
        .operation(
            Operation::new(WaitOperation::ID, async |_: &Context, _: WaitOperation| -> Result<Echoed> { std::future::pending().await })
                .visibility(Visibility::Public)
                .policy(wait),
        )
        .operation(
            Operation::new(FinishOperation::ID, async move |context: &Context, request: FinishOperation| {
                finish_owner.ran.fetch_add(1, Ordering::SeqCst);
                if request.mode == 2 {
                    return std::future::pending().await;
                }
                context.cancelled().await;
                if request.mode == 1 {
                    return std::future::pending().await;
                }
                Ok(Value { value: "42".into() })
            })
            .visibility(Visibility::Public)
            .policy(finish),
        )
        .requires::<ProviderEchoOperation>()
        .requires::<ReadOperation>()
        .requires::<harmos_protocol::host::Publish>()
        .stream(Stream::<Echoed>::new("progress"));
    Registration::new(plugin)
        .capacity(2)
        .lifecycle_policy(ordinary, ordinary)
        .on_initialize(async move |context: &Context, configuration: Vec<u8>| {
            let mode: String = serde_json::from_slice(&configuration).map_err(|error| Error::Invalid(error.to_string()))?;
            *initialization_owner.configuration.lock().unwrap() = mode.clone();
            *initialization_owner.initialized.lock().unwrap() = Some(context.clone());
            if mode == "dependencies" {
                context.operation(ProviderEchoOperation { say: "initialize".into() }).await?;
            }
            if mode == "publications" || mode == "failed-publications" {
                progress(context, "initialize").await?;
                if mode == "failed-publications" {
                    return Err(Error::Invalid("initialization refused after publication".into()));
                }
            }
            if mode == "submit" {
                submitted(context).await?;
            }
            Ok(())
        })
        .on_shutdown(async move |_: &Context| {
            let mode = shutdown_owner.configuration.lock().unwrap().clone();
            if mode == "shutdown-pending" {
                std::future::pending::<()>().await;
            }
            let saved = shutdown_owner
                .initialized
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| Error::Invalid("Initialization Context Is Missing".into()))?;
            if mode == "publications" && progress(&saved, "cleanup").await.is_ok() {
                return Err(Error::Invalid("Retirement Accepted a Publication".into()));
            }
            if mode == "dependencies" && saved.operation(ProviderEchoOperation { say: "cleanup".into() }).await.is_ok() {
                return Err(Error::Invalid("Retirement Accepted a Dependency Call".into()));
            }
            Ok(())
        })
        .on_cleanup(move || {
            cleanup_owner.initialized.lock().unwrap().take();
            Ok(())
        })
}

/* === Private Functions === */

fn policy(attempts: u32, delay_millis: i32) -> OperationPolicy {
    let deadline = Some(harmos::encoding::Duration { seconds: 2, nanos: 0 });
    OperationPolicy {
        timeout: deadline,
        attempt_timeout: deadline,
        retries: Some(harmos_protocol::v1::RetryPolicy {
            total_attempts: u64::from(attempts.max(1)),
            initial: (delay_millis > 0).then_some(harmos::encoding::Duration {
                seconds: 0,
                nanos: delay_millis * 1_000_000,
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
async fn progress(context: &Context, value: &str) -> Result<()> {
    context.publish(&Stream::new("progress"), Echoed { said: value.into() }).await
}
async fn echo(owner: &Owner, context: &Context, request: EchoOperation) -> Result<Echoed> {
    if let Some(value) = request.say.strip_prefix("saved-publish:") {
        let saved = owner.initialized.lock().unwrap().clone().ok_or_else(|| Error::Invalid("Initialization Context Is Missing".into()))?;
        progress(&saved, value).await?;
        return Ok(Echoed { said: format!("published {value}") });
    }
    if let Some(value) = request.say.strip_prefix("saved-dependency:") {
        let saved = owner.initialized.lock().unwrap().clone().ok_or_else(|| Error::Invalid("Initialization Context Is Missing".into()))?;
        return saved.operation(ProviderEchoOperation { say: value.into() }).await;
    }
    if let Some(value) = request.say.strip_prefix("saved-submit:") {
        let saved = owner.initialized.lock().unwrap().clone().ok_or_else(|| Error::Invalid("Initialization Context Is Missing".into()))?;
        let child = saved.submit(ProviderEchoOperation { say: value.into() }).await?;
        return child.join().await;
    }
    if let Some(say) = request.say.strip_prefix("dependency:") {
        let policy = context.value::<RetryChoice>()?.map(|choice| policy(choice.retries + 1, choice.delay_millis as i32));
        return context.operation_with(ProviderEchoOperation { say: say.into() }, policy).await;
    }
    if let Some(value) = request.say.strip_prefix("publish:") {
        progress(context, value).await?;
    }
    if request.say == "publish-many" {
        for index in 0..8 {
            progress(context, &index.to_string()).await?;
        }
    }
    if request.say == "grow" {
        std::hint::black_box(vec![1u8; 128 * 1024 * 1024]);
    }
    Ok(Echoed { said: format!("heard {}", request.say) })
}
async fn read(owner: &Owner, context: &Context, mut request: ReadOperation) -> Result<Value> {
    owner.ran.fetch_add(1, Ordering::SeqCst);
    if request.value == "context-leaf" {
        return Ok(Value {
            value: context.value::<Stage>()?.ok_or_else(|| Error::Invalid("Stage Is Missing".into()))?.id,
        });
    }
    if request.value == "context-parent" {
        let parent = context.value::<Stage>()?.ok_or_else(|| Error::Invalid("Stage Is Missing".into()))?;
        let child = context.with_value(Stage { id: "child".into() }).operation(ReadOperation { value: "context-leaf".into() }).await?;
        return Ok(Value {
            value: format!("{}:{}", parent.id, child.value),
        });
    }
    if request.value == "leaf" {
        return Ok(Value {
            value: format!("leaf:{}", context.attempt),
        });
    }
    let attempt = {
        let mut attempts = owner.attempts.lock().unwrap();
        let attempt = attempts.entry(request.value.clone()).or_default();
        *attempt += 1;
        *attempt
    };
    if request.value == "parent" {
        let saved = owner.saved.lock().unwrap().remove(&request.value);
        if let Some(saved) = saved
            && saved.operation(ReadOperation { value: "leaf".into() }).await.is_ok()
        {
            return Err(Error::Invalid("Stale Attempt Context Was Accepted".into()));
        }
        let child = context.operation(ReadOperation { value: "leaf".into() }).await?;
        if attempt > 1 {
            return Ok(Value {
                value: format!("parent:{attempt}:stale:{}", child.value),
            });
        }
        owner.saved.lock().unwrap().insert(request.value.clone(), context.clone());
    }
    if attempt == 1 {
        request.value.push_str(":mutated");
        return Err(Error::failure(Classification::Transient, "retry", "first transient attempt", "Retry"));
    }
    Ok(Value {
        value: format!("{}:{attempt}", request.value),
    })
}
async fn submitted(context: &Context) -> Result<()> {
    let request = |say: &str| ProviderEchoOperation { say: say.into() };
    let joined = context.submit(request("joined")).await?;
    let state = joined.status().await?;
    if state.phase != "running" {
        return Err(Error::Invalid("A Live Child Must Be Running".into()));
    }
    if joined.join().await?.said != "joined" {
        return Err(Error::Invalid("Joined Response Differs".into()));
    }
    let cancelled = context.submit(request("cancelled")).await?;
    if cancelled.cancel().await.is_ok() {
        return Err(Error::Invalid("A Cancelled Child Must Not Answer".into()));
    }
    let released = context.submit(request("released")).await?;
    released.release().await?;
    let dropped = context.submit(request("dropped")).await?;
    drop(dropped);
    context.submit(request("detached")).await?.detach().await?;
    Ok(())
}

/* === Tests === */
