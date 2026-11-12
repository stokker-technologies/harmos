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

#[path = "../../../../tools/conformance/src/schema_fixture.rs"]
mod schema_fixture;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos::{
    Classification, Context, Failure, Message, Observer, Operation, OperationRequest, Plugin, Publisher, Result, Stream, Visibility,
};
use schema_fixture::{Graph, Nested};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

use schema_fixture::Value;

#[derive(Message)]
#[harmos(package = "harmos.conformance.v1", file = "harmos/conformance/v1/fixture.proto")]
struct PayloadOnly {
    value: Vec<u8>,
}

#[derive(Message)]
#[harmos(package = "harmos.conformance.v1", file = "harmos/conformance/v1/fixture.proto", name = "Value")]
struct HostEcho {
    value: Vec<u8>,
}

#[derive(Message)]
#[harmos(package = "harmos.conformance.v1", file = "harmos/conformance/v1/fixture.proto", name = "Value")]
struct HostPending {
    value: Vec<u8>,
}

#[derive(Message)]
#[harmos(package = "harmos.conformance.v1", file = "harmos/conformance/v1/fixture.proto", name = "Value")]
struct HostFail {
    value: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Message for Value {
    fn name() -> String {
        "harmos.conformance.v1.Value".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        schema_fixture::types()
    }
}
impl Message for Graph {
    fn name() -> String {
        "harmos.conformance.v1.Graph".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        schema_fixture::types()
    }
}
impl Message for Nested {
    fn name() -> String {
        "harmos.conformance.v1.Graph.Nested".into()
    }
    fn types() -> prost_types::FileDescriptorSet {
        schema_fixture::types()
    }
}

impl OperationRequest for Graph {
    type Response = Value;
    const ID: &'static str = "conformance.host.schema";
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

fn main() -> Result<()> {
    let ticks = Stream::<Value>::new("conformance.ticks");
    let observer = Observer::new(&ticks, tick).every(prost_types::Duration { seconds: 1, nanos: 0 });
    let device = Stream::<Value>::new("conformance.device").publish_on(device);
    let settings = Stream::<Value>::new("conformance.settings");
    let configured = Observer::new(&settings, tick)
        .every(prost_types::Duration { seconds: 1, nanos: 0 })
        .interval_setting("harmos.conformance.v1.Graph.interval");
    let plugin = Plugin::new("conformance", "1.0.0")
        .message::<PayloadOnly>()
        .message::<PayloadOnly>()
        .operation(Operation::new("conformance.echo", echo).visibility(Visibility::Public))
        .operation(Operation::new("conformance.children", children).visibility(Visibility::Public))
        .operation(Operation::new("conformance.cancel", cancel).visibility(Visibility::Public))
        .operation(Operation::new("conformance.failure", failure).visibility(Visibility::Public))
        .operation(Operation::new("conformance.child_failure", child_failure).visibility(Visibility::Public))
        .operation(Operation::new("conformance.policy", policy).visibility(Visibility::Public))
        .operation(Operation::new("conformance.schema", schema).visibility(Visibility::Public))
        .operation(Operation::new("conformance.nested", nested).visibility(Visibility::Public))
        .operation(Operation::new("conformance.projected", projected).visibility(Visibility::Public))
        .operation(Operation::new("conformance.clock", clock).visibility(Visibility::Public))
        .operation(Operation::new("conformance.private", echo))
        .operation(Operation::new("conformance.resources", resources).visibility(Visibility::Public).replay_safe())
        .operation(Operation::new("conformance.cancel_reason", cancel_reason).visibility(Visibility::Public))
        .requires::<HostEcho>()
        .requires::<HostPending>()
        .requires::<HostFail>()
        .requires::<Graph>()
        .requires::<harmos::clock::TimeSleepRequest>()
        .requires::<harmos::clock::TimeGetRequest>()
        .stream(ticks)
        .stream(device)
        .stream(settings)
        .observer(observer)
        .observer(configured)
        .section(Graph {
            interval: Some(prost_types::Duration { seconds: 2, nanos: 0 }),
            ..Default::default()
        });
    if std::env::args().any(|argument| argument == "--remote") { plugin.remote().serve() } else { plugin.serve() }
}

async fn clock(ctx: &Context, _: Value) -> Result<Value> {
    let before = ctx.now().await?;
    ctx.sleep(std::time::Duration::from_secs(3600)).await?;
    let after = ctx.now().await?;
    Ok(Value {
        value: (after.seconds - before.seconds).to_le_bytes().to_vec(),
    })
}

async fn policy(ctx: &Context, request: Value) -> Result<Value> {
    let first = ctx
        .call_with(
            HostEcho { value: request.value },
            Some(harmos_protocol::v1::OperationPolicy {
                retries: Some(harmos_protocol::v1::RetryPolicy {
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
        next: Some(Box::new(request)),
    })
}

async fn nested(_: &Context, request: Nested) -> Result<Graph> {
    Ok(*request.next.unwrap_or_default())
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

async fn tick(ctx: &Context) -> Result<Value> {
    Ok(Value {
        value: ctx.sequence().to_le_bytes().to_vec(),
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
