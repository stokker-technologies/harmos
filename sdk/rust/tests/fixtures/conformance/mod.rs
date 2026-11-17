/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Rust Plugin SDK

/* === Modules === */

mod wire;
pub use wire::{Graph, PayloadOnly, Value, Values, graph};

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{future::Future, num::NonZeroUsize, task::Poll, time::Duration};

// Third Party Imports
use prost::Message;
use prost_types::FileDescriptorSet;

// Local Imports
use graph::Nested;
use harmos::model::Failure;
use harmos::plugin::HostOperation;
use harmos::plugin::TimeGetResponse;
use harmos::plugin::{Context, Observer, Operation, Plugin, Publisher, Registrar, Registration, Stream, Window};
use harmos::{Classification, OperationRequest, Result, Visibility};
use harmos::{SettingDeclaration, SettingValue, SettingsDeclarations, setting_value};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos::Message)]
#[harmos(package = "conformance.model.v1", file = "conformance/model/v1/fixture.proto", name = "Value")]
struct HostEcho {
    value: Vec<u8>,
}

#[derive(harmos::Message)]
#[harmos(package = "harmos.host.v1", file = "harmos/host/v1/invocations.proto", name = "InvocationsCreateRequest")]
struct CreateInvocation {
    operation: String,
    fingerprint: Vec<u8>,
    payload: Vec<u8>,
}

#[derive(harmos::Message)]
#[harmos(package = "harmos.host.v1", file = "harmos/host/v1/invocations.proto", name = "InvocationsCreateResponse")]
struct CreatedInvocation {
    payload: Vec<u8>,
}

#[derive(harmos::Message)]
#[harmos(package = "conformance.model.v1", file = "conformance/model/v1/fixture.proto", name = "Value")]
struct HostPending {
    value: Vec<u8>,
}

#[derive(harmos::Message)]
#[harmos(package = "conformance.model.v1", file = "conformance/model/v1/fixture.proto", name = "Value")]
struct HostFail {
    value: Vec<u8>,
}

#[derive(harmos::Message)]
#[harmos(package = "conformance.model.v1", file = "conformance/model/v1/fixture.proto", name = "Value")]
struct HostWatch {
    value: Vec<u8>,
}

#[derive(harmos::Message)]
#[harmos(package = "conformance.model.v1", file = "conformance/model/v1/fixture.proto", name = "Value")]
struct LocalInput {
    value: Vec<u8>,
}

macro_rules! peer_request {
    ($name:ident, $id:literal) => {
        #[derive(harmos::Message)]
        #[harmos(package = "conformance.model.v1", file = "conformance/model/v1/fixture.proto", name = "Value")]
        struct $name {
            value: Vec<u8>,
        }
        impl OperationRequest for $name {
            type Response = Value;
            const ID: &'static str = $id;
        }
    };
}
peer_request!(PeerPublic, "conformance.peer.public");
peer_request!(PeerProtected, "conformance.peer.protected");
peer_request!(PeerLocal, "conformance.peer.local");
peer_request!(OwnProtected, "example.plugin.v1.Lifecycle/Initialize");

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl OperationRequest for CreateInvocation {
    type Response = CreatedInvocation;
    const ID: &'static str = "application.host.v1.Invocations/Create";
}

impl harmos::Message for Value {
    fn name() -> String {
        "conformance.model.v1.Value".into()
    }
    fn types() -> FileDescriptorSet {
        wire::types()
    }
}
impl harmos::Message for Values {
    fn name() -> String {
        "conformance.model.v1.Values".into()
    }
    fn types() -> FileDescriptorSet {
        wire::types()
    }
}
impl harmos::Message for PayloadOnly {
    fn name() -> String {
        "conformance.model.v1.PayloadOnly".into()
    }
    fn types() -> FileDescriptorSet {
        wire::types()
    }
}
impl harmos::Message for Graph {
    fn name() -> String {
        "conformance.model.v1.Graph".into()
    }
    fn types() -> FileDescriptorSet {
        wire::types()
    }
}
impl harmos::Message for Nested {
    fn name() -> String {
        "conformance.model.v1.Graph.Nested".into()
    }
    fn types() -> FileDescriptorSet {
        wire::types()
    }
}

impl OperationRequest for Graph {
    type Response = Value;
    const ID: &'static str = "conformance.host.schema";
}

impl OperationRequest for HostWatch {
    const WATCH: bool = true;
    type Response = Value;
    const ID: &'static str = "conformance.host.watch";
}

impl OperationRequest for LocalInput {
    type Response = Value;
    const ID: &'static str = "conformance.local";
}

impl OperationRequest for HostEcho {
    type Response = Value;
    const ID: &'static str = "conformance.host.echo";
}
impl OperationRequest for HostPending {
    type Response = Value;
    const ID: &'static str = "conformance.host.pending";
}
impl OperationRequest for HostFail {
    type Response = Value;
    const ID: &'static str = "conformance.host.fail";
}

/* === Public Functions === */

/* === Private Functions === */

pub fn plugin() -> Plugin {
    Plugin::new("conformance", "1.0.0").register(|ctx| ctx.register(catalog())).expect("Register Plugin Declarations")
}

pub fn catalog() -> impl Registration {
    let interval = Stream::<Value>::new("conformance.interval");
    let observer = Observer::new(&interval, sequence).every(prost_types::Duration { seconds: 1, nanos: 0 });
    let device = Stream::<Value>::new("conformance.device").publish_on(device);
    let settings = Stream::<Value>::new("conformance.settings");
    let configured = Observer::new(&settings, sequence).every(prost_types::Duration { seconds: 1, nanos: 0 }).interval_setting("interval");
    let children_stream = Stream::<Value>::new("conformance.window_children");
    let children_observer = Observer::new(&children_stream, window_children).every(prost_types::Duration { seconds: 1, nanos: 0 });
    let context_stream = Stream::<Value>::new("conformance.window_context");
    let context_observer = Observer::new(&context_stream, window_context).every(prost_types::Duration { seconds: 1, nanos: 0 });
    let failure_stream = Stream::<Value>::new("conformance.window_failure");
    let failure_observer = Observer::new(&failure_stream, window_failure).every(prost_types::Duration { seconds: 1, nanos: 0 });
    let watch_stream = Stream::<Value>::new("conformance.window_watch");
    let watch_observer = Observer::new(&watch_stream, window_watch).every(prost_types::Duration { seconds: 1, nanos: 0 });
    let cancel_stream = Stream::<Value>::new("conformance.window_cancel");
    let cancel_observer = Observer::new(&cancel_stream, window_cancel).every(prost_types::Duration { seconds: 1, nanos: 0 });
    let refusal_stream = Stream::<Value>::new("conformance.window_caller_refusal");
    let refusal_observer = Observer::new(&refusal_stream, window_caller_refusal).every(prost_types::Duration { seconds: 1, nanos: 0 });
    let window_stream = Stream::<Value>::new("conformance.window");
    let window_observer = Observer::window(&window_stream, window).every(prost_types::Duration { seconds: 1, nanos: 0 });
    let oversized_stream = Stream::<Value>::new("conformance.oversized");
    let oversized_observer = Observer::new(&oversized_stream, oversized).every(prost_types::Duration { seconds: 1, nanos: 0 });
    move |ctx: &mut Registrar<'_>| {
        ctx.register(SettingsDeclarations {
            values: vec![
                SettingDeclaration {
                    name: "enabled".into(),
                    title: "Enabled".into(),
                    description: String::new(),
                    default: Some(SettingValue {
                        value: Some(setting_value::Value::Flag(false)),
                    }),
                },
                SettingDeclaration {
                    name: "count".into(),
                    title: "Count".into(),
                    description: String::new(),
                    default: Some(SettingValue {
                        value: Some(setting_value::Value::Integer(3)),
                    }),
                },
                SettingDeclaration {
                    name: "gain".into(),
                    title: "Gain".into(),
                    description: String::new(),
                    default: Some(SettingValue {
                        value: Some(setting_value::Value::Number(1.5)),
                    }),
                },
                SettingDeclaration {
                    name: "label".into(),
                    title: "Label".into(),
                    description: String::new(),
                    default: Some(SettingValue {
                        value: Some(setting_value::Value::Text("default".into())),
                    }),
                },
                SettingDeclaration {
                    name: "interval".into(),
                    title: "Interval".into(),
                    description: String::new(),
                    default: Some(SettingValue {
                        value: Some(setting_value::Value::Duration(prost_types::Duration { seconds: 2, nanos: 0 })),
                    }),
                },
            ],
        })?;
        ctx.register(harmos::plugin::Descriptor::<PayloadOnly>::new())?;
        ctx.register(Operation::new("conformance.local", local).visibility(Visibility::Private).replay_safe())?;
        ctx.register(Operation::new("conformance.local_call", local_call).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.step.0", echo).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.children", children).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.cancel", cancel).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.failure", failure).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.child_failure", child_failure).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.policy", policy).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.schema", schema).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.nested", nested).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.projected", projected).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.clock", clock).visibility(Visibility::Public))?;
        ctx.register(Operation::new("example.plugin.v1.Lifecycle/Initialize", echo).visibility(harmos::Visibility::Protected))?;
        ctx.register(Operation::new("example.plugin.v1.Steps/Execute", resources).visibility(Visibility::Public).replay_safe())?;
        ctx.register(Operation::new("conformance.cancel_reason", cancel_reason).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.watch", watch).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.peer_call", peer_call).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.settings_read", settings_read).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.discover", discover).visibility(Visibility::Public))?;
        ctx.register(Operation::new("conformance.caller_refusal", caller_refusal).visibility(Visibility::Public))?;
        ctx.register(harmos::plugin::Dependency::<CreateInvocation>::new())?;
        ctx.register(harmos::plugin::Dependency::<PeerPublic>::new())?;
        ctx.register(harmos::plugin::Dependency::<PeerProtected>::new())?;
        ctx.register(harmos::plugin::Dependency::<OwnProtected>::new())?;
        ctx.register(harmos::plugin::Dependency::<PeerLocal>::new())?;
        ctx.register(harmos::plugin::Dependency::<HostWatch>::new())?;
        ctx.register(harmos::plugin::Dependency::<HostEcho>::new())?;
        ctx.register(harmos::plugin::Dependency::<HostPending>::new())?;
        ctx.register(harmos::plugin::Dependency::<HostFail>::new())?;
        ctx.register(harmos::plugin::Dependency::<Graph>::new())?;
        ctx.register(HostOperation::TimeSleep)?;
        ctx.register(HostOperation::TimeGet)?;
        ctx.register(HostOperation::PluginsSettingsList)?;
        ctx.register(HostOperation::OperationsList)?;
        ctx.register(HostOperation::OperationsGet)?;
        ctx.register(interval)?;
        ctx.register(device)?;
        ctx.register(settings)?;
        ctx.register(children_stream)?;
        ctx.register(children_observer)?;
        ctx.register(context_stream)?;
        ctx.register(context_observer)?;
        ctx.register(failure_stream)?;
        ctx.register(failure_observer)?;
        ctx.register(watch_stream)?;
        ctx.register(watch_observer)?;
        ctx.register(cancel_stream)?;
        ctx.register(cancel_observer)?;
        ctx.register(refusal_stream)?;
        ctx.register(refusal_observer)?;
        ctx.register(window_stream)?;
        ctx.register(window_observer)?;
        ctx.register(oversized_stream)?;
        ctx.register(oversized_observer)?;
        ctx.register(observer)?;
        ctx.register(configured)?;
        ctx.register(harmos::plugin::Contribution(Graph {
            interval: Some(prost_types::Duration { seconds: 2, nanos: 0 }),
            ..Default::default()
        }))?;
        Ok(())
    }
}

async fn caller_refusal(ctx: &Context, value: Value) -> Result<Value> {
    let request = <CreateInvocation as prost::Message>::decode(include_bytes!("caller-request.bin").as_slice())
        .map_err(|error| harmos::Error::Invalid(format!("Create Invocation: {error}")))?;
    match ctx.call(request).await {
        Err(harmos::Error::Failure(failure)) if value.value != b"propagate" => Ok(Value { value: failure.encode_to_vec() }),
        Err(error) => Err(error),
        Ok(_) => Err(harmos::Error::Invalid("Create Invocation: Client-Only Operation Was Admitted".into())),
    }
}

async fn window_caller_refusal(ctx: &Context) -> Result<Value> {
    caller_refusal(ctx, Value::default()).await
}

async fn local_call(ctx: &Context, request: Value) -> Result<Value> {
    let policy = match request.value.as_slice() {
        b"retry" => Some(harmos::OperationPolicy {
            retries: Some(harmos::RetryPolicy {
                total_attempts: 2,
                initial: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
                maximum: Some(prost_types::Duration { seconds: 4, nanos: 0 }),
                deterministic_jitter: true,
            }),
            ..Default::default()
        }),
        b"deadline" => Some(harmos::OperationPolicy {
            timeout: Some(prost_types::Duration { seconds: 3, nanos: 0 }),
            attempt_timeout: Some(prost_types::Duration { seconds: 2, nanos: 0 }),
            ..Default::default()
        }),
        b"overall" => Some(harmos::OperationPolicy {
            timeout: Some(prost_types::Duration { seconds: 2, nanos: 0 }),
            cancellation_timeout: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
            ..Default::default()
        }),
        b"retry-bound" => Some(harmos::OperationPolicy {
            timeout: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
            retries: Some(harmos::RetryPolicy {
                total_attempts: 2,
                initial: Some(prost_types::Duration { seconds: 3600, nanos: 0 }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        b"cancel" => Some(harmos::OperationPolicy {
            cancellation_timeout: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
            ..Default::default()
        }),
        _ => None,
    };
    ctx.call_with(LocalInput { value: request.value }, policy).await
}

async fn clock(ctx: &Context, _: Value) -> Result<Value> {
    let before = ctx.now().await?;
    ctx.sleep(Duration::from_secs(3600)).await?;
    let after = ctx.now().await?;
    Ok(Value {
        value: (after.seconds - before.seconds).to_le_bytes().to_vec(),
    })
}

async fn policy(ctx: &Context, request: Value) -> Result<Value> {
    let first = ctx
        .call_with(
            HostEcho { value: request.value },
            Some(harmos::OperationPolicy {
                retries: Some(harmos::RetryPolicy {
                    total_attempts: 3,
                    initial: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
                    maximum: Some(prost_types::Duration { seconds: 4, nanos: 0 }),
                    deterministic_jitter: true,
                }),
                ..Default::default()
            }),
        )
        .await?;
    ctx.call(HostEcho { value: first.value }).await
}

async fn projected(_: &Context, request: Graph) -> Result<Value> {
    Ok(request.tail.and_then(|tail| tail.value).unwrap_or_default())
}

async fn schema(_: &Context, request: Graph) -> Result<Nested> {
    Ok(Nested {
        count: Some(request.items.len() as i32),
        next: Some(request),
    })
}

async fn nested(_: &Context, request: Nested) -> Result<Graph> {
    Ok(request.next.unwrap_or_default())
}

async fn echo(ctx: &Context, request: Value) -> Result<Value> {
    if request.value == b"scope" {
        return Ok(Value { value: ctx.scope()?.as_bytes().to_vec() });
    }
    Ok(request)
}

async fn children(ctx: &Context, _: Value) -> Result<Value> {
    let first = ctx.call(HostEcho { value: vec![1] });
    let second = ctx.call(HostEcho { value: vec![2] });
    let (first, second) = futures_util::future::try_join(first, second).await?;
    Ok(Value {
        value: [first.value, second.value].concat(),
    })
}

async fn cancel(ctx: &Context, request: Value) -> Result<Value> {
    ctx.call(HostPending { value: request.value }).await
}

async fn child_failure(ctx: &Context, request: Value) -> Result<Value> {
    ctx.call(HostFail { value: request.value }).await
}

async fn failure(_: &Context, _: Value) -> Result<Value> {
    Err(Failure {
        classification: Classification::Permanent as i32,
        code: "fixture_failure".into(),
        message: "Fixture Failed".into(),
        action: "Use Another Input".into(),
        operation: "conformance.failure".into(),
        plugin: "conformance".into(),
        cause: Some(Box::new(Failure {
            classification: Classification::Transient as i32,
            code: "fixture_cause".into(),
            message: "Fixture Cause".into(),
            action: "Retry".into(),
            operation: "conformance.host.fail".into(),
            plugin: "host".into(),
            ..Default::default()
        })),
        details: std::collections::HashMap::from([("fixture".into(), "value".into())]),
        diagnostics: "Fixture Diagnostics".into(),
    }
    .into())
}

async fn sequence(ctx: &Context) -> Result<Value> {
    Ok(Value {
        value: ctx.sequence().to_le_bytes().to_vec(),
    })
}

/// Each Sample Holds as Many Bytes as Its Sequence, So a Grant Can Exceed the harmos::Message Size.
async fn oversized(ctx: &Context) -> Result<Value> {
    Ok(Value { value: vec![0; ctx.sequence() as usize] })
}

async fn window(_: &Context, window: &Window) -> Result<Values> {
    Ok(Values {
        value: (0..window.count).map(|index| (window.sequence + index).to_le_bytes().to_vec()).collect(),
    })
}

async fn device(ctx: &Context, publisher: Publisher<Value>) -> Result<()> {
    publisher.publish(Value { value: b"device".to_vec() }).await?;
    ctx.cancelled().await;
    Ok(())
}

/* === Tests === */

async fn resources(ctx: &Context, request: Value) -> Result<Value> {
    ctx.set(request.clone());
    ctx.call(HostEcho { value: request.value }).await?;
    Ok((*ctx.get::<Value>()?).clone())
}

async fn cancel_reason(ctx: &Context, request: Value) -> Result<Value> {
    ctx.call(HostEcho { value: request.value }).await?;
    Ok(Value {
        value: ctx.cancelled().await.into_bytes(),
    })
}

async fn watch(context: &Context, request: Value) -> Result<Value> {
    let mut watching = context.watch(HostWatch { value: request.value.clone() }, std::num::NonZeroUsize::new(4096).unwrap()).await?;
    let mut value = Vec::new();
    while let Some(item) = watching.next().await? {
        value.extend(item.value);
        if request.value == b"cancel" {
            break;
        }
    }
    Ok(Value { value })
}

async fn local(context: &Context, request: Value) -> Result<Value> {
    if matches!(request.value.as_slice(), b"deadline" | b"overall") {
        context.sleep(Duration::from_secs(3600)).await?;
        return Ok(request);
    }
    if request.value == b"cancel" {
        context.call(HostEcho { value: request.value }).await?;
        return Ok(Value {
            value: context.cancelled().await.into_bytes(),
        });
    }
    if !matches!(request.value.as_slice(), b"retry" | b"retry-bound") {
        return Ok(request);
    }
    if context.attempt == 1 {
        return Err(harmos::Error::failure(Classification::Transient, "retry", "Retry", "Invoke Again"));
    }
    Ok(Value {
        value: context.attempt.to_string().into_bytes(),
    })
}

async fn peer_call(context: &Context, request: Value) -> Result<Value> {
    match request.value.as_slice() {
        b"public" => {
            context
                .call(PeerPublic {
                    value: context.scope()?.as_bytes().to_vec(),
                })
                .await
        }
        b"protected" => context.call(PeerProtected { value: request.value }).await,
        b"self" => context.call(OwnProtected { value: request.value }).await,
        _ => context.call(PeerLocal { value: request.value }).await,
    }
}

async fn settings_read(context: &Context, _: Value) -> Result<Value> {
    let settings = context.settings().list().await?.value.ok_or_else(|| harmos::Error::Invalid("List Settings: Values Missing".into()))?;
    Ok(Value { value: settings.encode_to_vec() })
}

async fn discover(context: &Context, _: Value) -> Result<Value> {
    let first = context
        .host()
        .operations
        .list()
        .await?
        .into_iter()
        .next()
        .and_then(|entry| entry.contract)
        .ok_or_else(|| harmos::Error::Invalid("List Operations: Contract Missing".into()))?;
    let selected = context
        .host()
        .operations
        .get(first.id.clone())
        .await?
        .contract
        .ok_or_else(|| harmos::Error::Invalid("Get Operation: Contract Missing".into()))?;
    if first != selected {
        return Err(harmos::Error::Invalid("Get Operation: Metadata Changed".into()));
    }
    Ok(Value { value: first.id.into_bytes() })
}

async fn window_children(context: &Context) -> Result<Value> {
    let (first, second) = tokio::join!(context.call(HostEcho { value: vec![1] }), context.call(HostEcho { value: vec![2] }));
    let mut value = first?.value;
    value.extend(second?.value);
    Ok(Value { value })
}

async fn window_context(context: &Context) -> Result<Value> {
    let echo = context
        .call(HostEcho {
            value: context.scope()?.as_bytes().to_vec(),
        })
        .await?
        .encode_to_vec();
    let clock = TimeGetResponse {
        now: Some(context.host().time.get().await?),
    }
    .encode_to_vec();
    let settings = context.settings().list().await?.encode_to_vec();
    let mut value = Vec::new();
    for payload in [echo, clock, settings] {
        let length = u32::try_from(payload.len()).map_err(|error| harmos::Error::Invalid(format!("Encode Window Context: {error}")))?;
        value.extend(length.to_le_bytes());
        value.extend(payload);
    }
    Ok(Value { value })
}

async fn window_failure(context: &Context) -> Result<Value> {
    match context.call(HostFail { value: Vec::new() }).await {
        Err(harmos::Error::Failure(failure)) => Ok(Value { value: failure.encode_to_vec() }),
        Err(error) => Err(error),
        Ok(_) => Err(harmos::Error::Invalid("Call Failure Fixture: Expected Failure".into())),
    }
}

async fn window_watch(context: &Context) -> Result<Value> {
    let mut watching = context
        .watch(
            HostWatch {
                value: context.scope()?.as_bytes().to_vec(),
            },
            NonZeroUsize::new(4096).unwrap(),
        )
        .await?;
    let mut value = Vec::new();
    while let Some(item) = watching.next().await? {
        value.extend(item.value);
    }
    Ok(Value { value })
}

async fn window_cancel(context: &Context) -> Result<Value> {
    {
        let pending = context.call(HostPending { value: Vec::new() });
        let release = context.call(HostEcho { value: b"release".to_vec() });
        tokio::pin!(pending, release);
        tokio::select! {
            biased;
            result = &mut pending => { result?; return Err(harmos::Error::Invalid("Cancel Window Child: Pending Child Answered".into())); },
            result = &mut release => { result?; },
        }
    }
    let pending = context.call(HostPending { value: Vec::new() });
    tokio::pin!(pending);
    futures_util::future::poll_fn(|cx| match pending.as_mut().poll(cx) {
        Poll::Ready(result) => Poll::Ready(result.map(|_| ())),
        Poll::Pending => Poll::Ready(Ok(())),
    })
    .await?;
    Ok(Value { value: b"cancelled".to_vec() })
}
