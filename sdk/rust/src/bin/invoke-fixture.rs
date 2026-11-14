/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Runtime Invoke Story Fixture

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    env, fs,
    num::NonZeroUsize,
    result::Result as StdResult,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

// Third Party Imports
use prost::{
    DecodeError, Message as _,
    bytes::{Buf, BufMut},
    encoding::{DecodeContext, WireType},
};
use prost_reflect::DescriptorPool;
use prost_types::{Duration as ProtoDuration, FileDescriptorSet, Timestamp};

// Local Imports
use harmos::{Context, Message, Operation, OperationRequest, Plugin, Result, Visibility};
use harmos_protocol::v1;

/* === Constants === */

static DEVICE_ACTIVE: AtomicBool = AtomicBool::new(false);

static HOST: OnceLock<(FileDescriptorSet, String, String, u32, u32)> = OnceLock::new();

/* === Macros === */

macro_rules! host_message {
    ($type:ty, $index:tt) => {
        impl prost::Message for $type {
            fn encode_raw(&self, buf: &mut impl BufMut) {
                prost::encoding::bytes::encode(HOST.get().unwrap().$index, &self.value, buf);
            }
            fn merge_field(&mut self, tag: u32, wire: WireType, buf: &mut impl Buf, context: DecodeContext) -> StdResult<(), DecodeError> {
                if tag == HOST.get().unwrap().$index {
                    prost::encoding::bytes::merge(wire, &mut self.value, buf, context)
                } else {
                    prost::encoding::skip_field(wire, tag, buf, context)
                }
            }
            fn encoded_len(&self) -> usize {
                prost::encoding::bytes::encoded_len(HOST.get().unwrap().$index, &self.value)
            }
            fn clear(&mut self) {
                self.value.clear();
            }
        }
    };
}
host_message!(HostInput, 3);
host_message!(HostOutput, 4);

/* === Type Aliases === */

/* === Structs === */

#[derive(Message)]
struct Value {
    value: Vec<u8>,
}

#[derive(Debug, Default)]
struct HostInput {
    value: Vec<u8>,
}

#[derive(Debug, Default)]
struct HostOutput {
    value: Vec<u8>,
}

#[derive(prost::Message)]
struct PublicInput {
    #[prost(bytes = "vec", tag = "1")]
    value: Vec<u8>,
}

#[derive(prost::Message)]
struct ProtectedInput {
    #[prost(bytes = "vec", tag = "1")]
    value: Vec<u8>,
}

#[derive(prost::Message)]
struct MissingInput {
    #[prost(bytes = "vec", tag = "1")]
    value: Vec<u8>,
}

struct DeviceDemand;

impl Drop for DeviceDemand {
    fn drop(&mut self) {
        DEVICE_ACTIVE.store(false, Ordering::SeqCst);
    }
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Message for HostInput {
    fn name() -> String {
        HOST.get().unwrap().1.clone()
    }
    fn types() -> FileDescriptorSet {
        HOST.get().unwrap().0.clone()
    }
}
impl Message for HostOutput {
    fn name() -> String {
        HOST.get().unwrap().2.clone()
    }
    fn types() -> FileDescriptorSet {
        HOST.get().unwrap().0.clone()
    }
}
#[derive(harmos::Message)]
#[harmos(package = "harmos.plugin.host.v1", file = "harmos/plugin/host/v1/time.proto", name = "TimeGetRequest")]
struct HostControl {}
impl OperationRequest for HostControl {
    type Response = harmos::clock::TimeGetResponse;
    const ID: &'static str = "fixture.control";
}

#[derive(harmos::Message)]
#[harmos(package = "harmos.client.v1", file = "harmos/client/v1/time.proto", name = "TimeGetRequest")]
struct ClientTime {}
#[derive(harmos::Message)]
#[harmos(package = "harmos.client.v1", file = "harmos/client/v1/time.proto", name = "TimeGetResponse")]
struct ClientTimeResponse {
    now: Option<Timestamp>,
}
impl OperationRequest for ClientTime {
    type Response = ClientTimeResponse;
    const ID: &'static str = "harmos.client.v1.Time/Get";
}

#[derive(harmos::Message)]
#[harmos(package = "harmos.plugin.host.v1", file = "harmos/plugin/host/v1/time.proto", name = "TimeGetRequest")]
struct ApplicationContext {}
impl OperationRequest for ApplicationContext {
    type Response = harmos::clock::TimeGetResponse;
    const ID: &'static str = "example.plugin.host.v1.Prompts/Create";
}

#[derive(harmos::Message)]
#[harmos(package = "harmos.plugin.host.v1", file = "harmos/plugin/host/v1/time.proto", name = "TimeGetRequest")]
struct ApplicationClient {}
impl OperationRequest for ApplicationClient {
    type Response = harmos::clock::TimeGetResponse;
    const ID: &'static str = "example.client.v1.Steps/Execute";
}

#[derive(harmos::Message)]
#[harmos(package = "harmos.plugin.host.v1", file = "harmos/plugin/host/v1/time.proto", name = "TimeGetRequest")]
struct ApplicationInstance {}
impl OperationRequest for ApplicationInstance {
    type Response = harmos::clock::TimeGetResponse;
    const ID: &'static str = "example.plugin.v1.Control/Execute";
}

impl OperationRequest for HostInput {
    type Response = HostOutput;
    const ID: &'static str = "fixture.host";
}
impl Message for PublicInput {
    fn name() -> String {
        Value::name()
    }
    fn types() -> FileDescriptorSet {
        Value::types()
    }
}
impl OperationRequest for PublicInput {
    type Response = Value;
    const ID: &'static str = "example.plugin.v1.Steps/Execute";
}
impl Message for ProtectedInput {
    fn name() -> String {
        Value::name()
    }
    fn types() -> FileDescriptorSet {
        Value::types()
    }
}
impl OperationRequest for ProtectedInput {
    type Response = Value;
    const ID: &'static str = "example.plugin.v1.Lifecycle/Initialize";
}
impl Message for MissingInput {
    fn name() -> String {
        Value::name()
    }
    fn types() -> FileDescriptorSet {
        Value::types()
    }
}
#[derive(prost::Message)]
struct PeerInput {
    #[prost(bytes = "vec", tag = "1")]
    value: Vec<u8>,
}
impl Message for PeerInput {
    fn name() -> String {
        Value::name()
    }
    fn types() -> FileDescriptorSet {
        Value::types()
    }
}
impl OperationRequest for PeerInput {
    type Response = Value;
    const ID: &'static str = "fixture.peer";
}

impl OperationRequest for MissingInput {
    type Response = Value;
    const ID: &'static str = "fixture.missing";
}

/* === Public Functions === */

/* === Private Functions === */

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let mode = args.next().unwrap();
    let types =
        FileDescriptorSet::decode(fs::read(args.next().unwrap())?.as_slice()).map_err(|error| harmos::Error::Invalid(error.to_string()))?;
    let request = args.next().unwrap();
    let response = args.next().unwrap();
    let pool = DescriptorPool::from_file_descriptor_set(types.clone()).unwrap();
    let request_tag = pool.get_message_by_name(&request).unwrap().get_field_by_name("value").unwrap().number();
    let response_tag = pool.get_message_by_name(&response).unwrap().get_field_by_name("value").unwrap().number();
    HOST.set((types, request, response, request_tag, response_tag)).unwrap();
    let plugin = if mode == "interval" {
        let stream = harmos::Stream::<Value>::new("fixture.ticks");
        let observer = harmos::Observer::new(&stream, observe).every(ProtoDuration { seconds: 60, nanos: 0 }).interval_setting("interval");
        Plugin::new("observer", "1.0.0")
            .settings(v1::SettingsDeclarations {
                values: vec![v1::SettingDeclaration {
                    name: "interval".into(),
                    title: "Interval".into(),
                    description: String::new(),
                    default: Some(v1::SettingValue {
                        value: Some(v1::setting_value::Value::Duration(ProtoDuration { seconds: 1, nanos: 0 })),
                    }),
                }],
            })
            .stream(stream)
            .observer(observer)
            .operation(Operation::new("fixture.echo", echo).visibility(Visibility::Protected))
    } else if mode == "retry" {
        Plugin::new("retry", "1.0.0").operation(
            Operation::new("fixture.retry", retry).visibility(Visibility::Protected).replay_safe().policy(v1::OperationPolicy {
                retries: Some(v1::RetryPolicy {
                    total_attempts: 3,
                    initial: Some(ProtoDuration { seconds: 2, nanos: 0 }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        )
    } else if mode == "deadline" {
        Plugin::new("deadline", "1.0.0")
            .requires::<harmos::clock::TimeGetRequest>()
            .requires::<harmos::clock::TimeSleepRequest>()
            .operation(Operation::new("fixture.clock", clock).visibility(Visibility::Protected).policy(v1::OperationPolicy {
                timeout: Some(ProtoDuration { seconds: 3, nanos: 0 }),
                attempt_timeout: Some(ProtoDuration { seconds: 2, nanos: 0 }),
                ..Default::default()
            }))
    } else if mode == "twins" {
        let left = harmos::Stream::<Value>::new("fixture.left");
        let right = harmos::Stream::<Value>::new("fixture.right");
        let left_observer = harmos::Observer::new(&left, observe).every(ProtoDuration { seconds: 1, nanos: 0 });
        let right_observer = harmos::Observer::new(&right, observe).every(ProtoDuration { seconds: 2, nanos: 0 });
        Plugin::new("twins", "1.0.0")
            .stream(left)
            .stream(right)
            .observer(left_observer)
            .observer(right_observer)
            .operation(Operation::new("fixture.echo", echo).visibility(Visibility::Protected))
    } else if mode == "stop-failure" {
        Plugin::new("stop_failure", "1.0.0")
            .operation(Operation::new("fixture.echo", echo).visibility(Visibility::Protected))
            .on_stop(|| Err(harmos::Error::Invalid("Cleanup Failed".into())))
    } else if mode == "device" {
        let stream = harmos::Stream::<Value>::new("fixture.device").publish_on(device);
        Plugin::new("device", "1.0.0")
            .stream(stream)
            .operation(Operation::new("fixture.echo", echo).visibility(Visibility::Protected))
            .on_stop(|| {
                if DEVICE_ACTIVE.load(Ordering::SeqCst) {
                    return Err(harmos::Error::Invalid("Device Demand Still Active".into()));
                }
                Ok(())
            })
    } else if mode == "scoped" {
        Plugin::new("scoped", "1.0.0")
            .requires::<HostInput>()
            .requires::<PublicInput>()
            .operation(Operation::new("fixture.scoped", scoped).visibility(Visibility::Protected))
    } else if mode == "scoped-provider" {
        Plugin::new("provider", "1.0.0")
            .operation(Operation::new("example.plugin.v1.Steps/Execute", scope_identity).visibility(Visibility::Public))
    } else if mode == "values" {
        Plugin::new("values", "1.0.0")
            .requires::<harmos_protocol::plugin::host::SettingsListRequest>()
            .message::<harmos::clock::TimeGetResponse>()
            .section(v1::SettingsDeclarations {
                values: vec![v1::SettingDeclaration {
                    name: "reading".into(),
                    title: "Reading".into(),
                    description: String::new(),
                    default: Some(v1::SettingValue {
                        value: Some(v1::setting_value::Value::Integer(0)),
                    }),
                }],
            })
            .operation(Operation::new("fixture.value", setting_value).visibility(Visibility::Protected))
    } else if mode == "watch" {
        Plugin::new("watcher", "1.0.0")
            .requires::<HostInput>()
            .requires::<harmos_protocol::plugin::host::InvocationsWatchRequest>()
            .operation(Operation::new("fixture.watch", watch_invocation).visibility(Visibility::Protected))
    } else if mode == "provider" {
        Plugin::new("provider", "1.0.0")
            .operation(Operation::new("fixture.step.0", echo).visibility(Visibility::Public))
            .requires::<HostInput>()
            .requires::<ProtectedInput>()
            .operation(Operation::new("fixture.self", protected_self).visibility(Visibility::Protected))
            .operation(Operation::new("fixture.peer", peer).visibility(Visibility::Public).policy(v1::OperationPolicy {
                cancellation_timeout: Some(ProtoDuration { seconds: 30, nanos: 0 }),
                ..Default::default()
            }))
            .operation(Operation::new("example.plugin.v1.Steps/Execute", echo).visibility(Visibility::Public))
            .operation(Operation::new("example.plugin.v1.Lifecycle/Initialize", echo).visibility(Visibility::Protected))
    } else if mode == "incompatible" {
        Plugin::new("incompatible", "1.0.0")
            .operation(Operation::new("example.plugin.v1.Steps/Execute", empty).visibility(Visibility::Public))
    } else if mode == "duplicate" {
        Plugin::new("duplicate", "1.0.0").operation(Operation::new("fixture.echo", echo).visibility(Visibility::Protected))
    } else {
        Plugin::new("caller", "1.0.0")
            .requires::<HostInput>()
            .requires::<HostControl>()
            .requires::<ClientTime>()
            .requires::<ApplicationContext>()
            .requires::<ApplicationClient>()
            .requires::<ApplicationInstance>()
            .requires::<harmos::clock::TimeSleepRequest>()
            .requires::<harmos::clock::TimeGetRequest>()
            .requires::<PublicInput>()
            .requires::<PeerInput>()
            .operation(Operation::new("fixture.peer_forward", peer_forward).visibility(Visibility::Protected).policy(v1::OperationPolicy {
                cancellation_timeout: Some(ProtoDuration { seconds: 30, nanos: 0 }),
                ..Default::default()
            }))
            .requires::<ProtectedInput>()
            .requires::<MissingInput>()
            .operation(Operation::new("fixture.clock", clock).visibility(Visibility::Protected))
            .operation(Operation::new("fixture.echo", echo).visibility(Visibility::Protected))
            .operation(Operation::new("fixture.forward", forward).visibility(Visibility::Protected))
            .operation(Operation::new("fixture.failure", failure).visibility(Visibility::Protected))
            .operation(Operation::new("fixture.wait", wait).visibility(Visibility::Protected).policy(v1::OperationPolicy {
                cancellation_timeout: Some(ProtoDuration { seconds: 30, nanos: 0 }),
                ..Default::default()
            }))
    };
    if args.next().as_deref() == Some("--managed") { plugin.serve() } else { plugin.remote().serve() }
}

async fn observe(context: &Context) -> Result<Value> {
    Ok(Value {
        value: context.sequence().to_le_bytes().to_vec(),
    })
}

async fn device(context: &Context, publisher: harmos::Publisher<Value>) -> Result<()> {
    DEVICE_ACTIVE.store(true, Ordering::SeqCst);
    let _demand = DeviceDemand;
    publisher.publish(Value { value: b"device".to_vec() }).await?;
    publisher.publish_at(Value { value: b"source".to_vec() }, Some(Timestamp { seconds: 7, nanos: 0 })).await?;
    context.cancelled().await;
    Ok(())
}

async fn clock(context: &Context, _: Value) -> Result<Value> {
    let before = context.now().await?;
    context.sleep(Duration::from_secs(3600)).await?;
    let after = context.now().await?;
    Ok(Value {
        value: (after.seconds - before.seconds).to_le_bytes().to_vec(),
    })
}

async fn empty(_: &Context, _: ()) -> Result<()> {
    Ok(())
}

async fn scope_identity(context: &Context, _: Value) -> Result<Value> {
    Ok(Value {
        value: context.scope()?.as_bytes().to_vec(),
    })
}

async fn scoped(context: &Context, input: Value) -> Result<Value> {
    let scope = context.scope()?;
    if input.value != scope.as_bytes() {
        return Err(harmos::Error::Invalid("Payload Differs From Invocation Scope".into()));
    }
    let host = context.call(HostInput { value: b"scope".to_vec() }).await?;
    let plugin = context.call(PublicInput { value: Vec::new() }).await?;
    let mut value = scope.as_bytes().to_vec();
    value.push(0);
    value.extend_from_slice(&host.value);
    value.push(0);
    value.extend_from_slice(&plugin.value);
    Ok(Value { value })
}

async fn echo(_: &Context, input: Value) -> Result<Value> {
    Ok(input)
}

async fn protected_self(context: &Context, input: Value) -> Result<Value> {
    context.call(ProtectedInput { value: input.value }).await
}

async fn retry(context: &Context, input: Value) -> Result<Value> {
    if context.attempt < 3 {
        return Err(harmos::Error::failure(v1::Classification::Transient, "retry", "Retry", "Try Again"));
    }
    Ok(input)
}

async fn failure(_: &Context, _: Value) -> Result<Value> {
    Err(v1::Failure {
        classification: v1::Classification::Permanent as i32,
        code: "fixture_failure".into(),
        message: "Fixture Failed".into(),
        action: "Use Another Input".into(),
        details: [("fixture".into(), "value".into())].into(),
        diagnostics: "Fixture Diagnostics".into(),
        cause: Some(Box::new(v1::Failure {
            classification: v1::Classification::Transient as i32,
            code: "fixture_cause".into(),
            message: "Fixture Cause".into(),
            ..Default::default()
        })),
        ..Default::default()
    }
    .into())
}

async fn forward(context: &Context, input: Value) -> Result<Value> {
    if input.value == b"client-only" {
        context.call(ClientTime {}).await?;
        return Ok(input);
    }
    if input.value == b"control" {
        context.call(HostControl {}).await?;
        return Ok(input);
    }
    if input.value == b"application-context" {
        let response = context.call(ApplicationContext {}).await?;
        return Ok(Value {
            value: response.now.unwrap().seconds.to_le_bytes().to_vec(),
        });
    }
    if input.value == b"application-client" {
        context.call(ApplicationClient {}).await?;
        return Ok(input);
    }
    if input.value == b"application-instance" {
        context.call(ApplicationInstance {}).await?;
        return Ok(input);
    }
    match input.value.first() {
        Some(b'h') => Ok(Value {
            value: context.call(HostInput { value: input.value }).await?.value,
        }),
        Some(b'p') => context.call(PublicInput { value: input.value }).await,
        Some(b's') => context.call(ProtectedInput { value: input.value }).await,
        _ => context.call(MissingInput { value: input.value }).await,
    }
}

async fn wait(context: &Context, input: Value) -> Result<Value> {
    context.call(HostInput { value: input.value.clone() }).await?;
    context.cancelled().await;
    Ok(input)
}

/* === Tests === */

async fn watch_invocation(context: &Context, request: Value) -> Result<Value> {
    let instance =
        u64::from_le_bytes(request.value.as_slice().try_into().map_err(|_| harmos::Error::Invalid("Watch: Instance Required".into()))?);
    let mut watch =
        context.watch(harmos_protocol::plugin::host::InvocationsWatchRequest { instance }, NonZeroUsize::new(4096).unwrap()).await?;
    let mut phases = Vec::new();
    while let Some(state) = watch.next().await? {
        phases.extend(state.invocation.unwrap().phase.as_bytes());
        phases.push(b'|');
        if !phases[..phases.len() - 1].contains(&b'|') {
            context.call(HostInput { value: b"watch opened".to_vec() }).await?;
        }
    }
    Ok(Value { value: phases })
}

async fn setting_value(context: &Context, request: Value) -> Result<Value> {
    let name = String::from_utf8(request.value).map_err(|error| harmos::Error::Invalid(format!("List Settings: {error}")))?;
    let response = context.settings().await?;
    let value = response
        .value
        .and_then(|settings| settings.values.get(&name).cloned())
        .ok_or_else(|| harmos::Error::Invalid(format!("List Settings: Missing {name}")))?;
    Ok(Value { value: value.encode_to_vec() })
}

async fn peer_forward(context: &Context, input: Value) -> Result<Value> {
    match context.call(PeerInput { value: input.value.clone() }).await {
        Err(harmos::Error::Failure(failure)) if failure.classification() == v1::Classification::Cancelled => Ok(input),
        result => result,
    }
}
async fn peer(context: &Context, input: Value) -> Result<Value> {
    if input.value == b"failure" {
        return failure(context, input).await;
    }
    if input.value == b"wait" {
        return wait(context, input).await;
    }
    Ok(Value {
        value: context.scope()?.as_bytes().to_vec(),
    })
}
