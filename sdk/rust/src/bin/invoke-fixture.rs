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
use std::sync::OnceLock;

// Third Party Imports
use prost::Message as _;

// Local Imports
use harmos::{Context, Message, Operation, OperationRequest, Plugin, Result, Visibility};
use harmos_protocol::v1;

/* === Constants === */

static DEVICE_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

static HOST: OnceLock<(prost_types::FileDescriptorSet, String, String, u32, u32)> = OnceLock::new();

/* === Macros === */

macro_rules! host_message {
    ($type:ty, $index:tt) => {
        impl prost::Message for $type {
            fn encode_raw(&self, buf: &mut impl prost::bytes::BufMut) {
                prost::encoding::bytes::encode(HOST.get().unwrap().$index, &self.value, buf);
            }
            fn merge_field(
                &mut self, tag: u32, wire: prost::encoding::WireType, buf: &mut impl prost::bytes::Buf,
                context: prost::encoding::DecodeContext,
            ) -> std::result::Result<(), prost::DecodeError> {
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

#[derive(Message)]
struct Settings {
    interval: Option<prost_types::Duration>,
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
struct PrivateInput {
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
        DEVICE_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Message for HostInput {
    fn name() -> String {
        HOST.get().unwrap().1.clone()
    }
    fn types() -> prost_types::FileDescriptorSet {
        HOST.get().unwrap().0.clone()
    }
}
impl Message for HostOutput {
    fn name() -> String {
        HOST.get().unwrap().2.clone()
    }
    fn types() -> prost_types::FileDescriptorSet {
        HOST.get().unwrap().0.clone()
    }
}
impl OperationRequest for HostInput {
    type Response = HostOutput;
    const ID: &'static str = "fixture.host";
}
impl Message for PublicInput {
    fn name() -> String {
        Value::name()
    }
    fn types() -> prost_types::FileDescriptorSet {
        Value::types()
    }
}
impl OperationRequest for PublicInput {
    type Response = Value;
    const ID: &'static str = "fixture.public";
}
impl Message for PrivateInput {
    fn name() -> String {
        Value::name()
    }
    fn types() -> prost_types::FileDescriptorSet {
        Value::types()
    }
}
impl OperationRequest for PrivateInput {
    type Response = Value;
    const ID: &'static str = "fixture.private";
}
impl Message for MissingInput {
    fn name() -> String {
        Value::name()
    }
    fn types() -> prost_types::FileDescriptorSet {
        Value::types()
    }
}
impl OperationRequest for MissingInput {
    type Response = Value;
    const ID: &'static str = "fixture.missing";
}

/* === Public Functions === */

/* === Private Functions === */

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap();
    let types = prost_types::FileDescriptorSet::decode(std::fs::read(args.next().unwrap())?.as_slice())
        .map_err(|error| harmos::Error::Invalid(error.to_string()))?;
    let request = args.next().unwrap();
    let response = args.next().unwrap();
    let pool = prost_reflect::DescriptorPool::from_file_descriptor_set(types.clone()).unwrap();
    let request_tag = pool.get_message_by_name(&request).unwrap().get_field_by_name("value").unwrap().number();
    let response_tag = pool.get_message_by_name(&response).unwrap().get_field_by_name("value").unwrap().number();
    HOST.set((types, request, response, request_tag, response_tag)).unwrap();
    let plugin = if mode == "interval" {
        let stream = harmos::Stream::<Value>::new("fixture.ticks");
        let observer = harmos::Observer::new(&stream, observe).every_ms(60_000).interval_setting("harmos.plugin.v1.Settings.interval");
        Plugin::new("observer", "1.0.0")
            .section(Settings {
                interval: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
            })
            .stream(stream)
            .observer(observer)
            .operation(Operation::new("fixture.echo", echo))
    } else if mode == "retry" {
        Plugin::new("retry", "1.0.0").operation(Operation::new("fixture.retry", retry).policy(v1::OperationPolicy {
            attempts: 3,
            backoff: Some(prost_types::Duration { seconds: 2, nanos: 0 }),
            ..Default::default()
        }))
    } else if mode == "deadline" {
        Plugin::new("deadline", "1.0.0").requires::<harmos::clock::Now>().requires::<harmos::clock::Sleep>().operation(
            Operation::new("fixture.clock", clock).policy(v1::OperationPolicy {
                timeout: Some(prost_types::Duration { seconds: 3, nanos: 0 }),
                attempt_timeout: Some(prost_types::Duration { seconds: 2, nanos: 0 }),
                ..Default::default()
            }),
        )
    } else if mode == "twins" {
        let left = harmos::Stream::<Value>::new("fixture.left");
        let right = harmos::Stream::<Value>::new("fixture.right");
        let left_observer = harmos::Observer::new(&left, observe).every_ms(1000);
        let right_observer = harmos::Observer::new(&right, observe).every_ms(2000);
        Plugin::new("twins", "1.0.0")
            .stream(left)
            .stream(right)
            .observer(left_observer)
            .observer(right_observer)
            .operation(Operation::new("fixture.echo", echo))
    } else if mode == "stop-failure" {
        Plugin::new("stop_failure", "1.0.0")
            .operation(Operation::new("fixture.echo", echo))
            .on_stop(|| Err(harmos::Error::Invalid("Cleanup Failed".into())))
    } else if mode == "device" {
        let stream = harmos::Stream::<Value>::new("fixture.device").publish_on(device);
        Plugin::new("device", "1.0.0").stream(stream).operation(Operation::new("fixture.echo", echo)).on_stop(|| {
            if DEVICE_ACTIVE.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(harmos::Error::Invalid("Device Demand Still Active".into()));
            }
            Ok(())
        })
    } else if mode == "provider" {
        Plugin::new("provider", "1.0.0")
            .operation(Operation::new("fixture.public", echo).visibility(Visibility::Public))
            .operation(Operation::new("fixture.private", echo))
    } else if mode == "incompatible" {
        Plugin::new("incompatible", "1.0.0").operation(Operation::new("fixture.public", empty).visibility(Visibility::Public))
    } else if mode == "duplicate" {
        Plugin::new("duplicate", "1.0.0").operation(Operation::new("fixture.echo", echo))
    } else {
        Plugin::new("caller", "1.0.0")
            .requires::<HostInput>()
            .requires::<harmos::clock::Sleep>()
            .requires::<harmos::clock::Now>()
            .requires::<PublicInput>()
            .requires::<PrivateInput>()
            .requires::<MissingInput>()
            .operation(Operation::new("fixture.clock", clock))
            .operation(Operation::new("fixture.echo", echo))
            .operation(Operation::new("fixture.forward", forward))
            .operation(Operation::new("fixture.failure", failure))
            .operation(Operation::new("fixture.wait", wait).policy(v1::OperationPolicy {
                cancellation_timeout: Some(prost_types::Duration { seconds: 30, nanos: 0 }),
                ..Default::default()
            }))
    };
    plugin.remote().serve()
}

async fn observe(context: &Context) -> Result<Value> {
    Ok(Value {
        value: context.sequence().to_le_bytes().to_vec(),
    })
}

async fn device(context: &Context, publisher: harmos::Publisher<Value>) -> Result<()> {
    DEVICE_ACTIVE.store(true, std::sync::atomic::Ordering::SeqCst);
    let _demand = DeviceDemand;
    publisher.publish(Value { value: b"device".to_vec() }).await?;
    publisher.publish_at(Value { value: b"source".to_vec() }, Some(prost_types::Timestamp { seconds: 7, nanos: 0 })).await?;
    context.cancelled().await;
    Ok(())
}

async fn clock(context: &Context, _: Value) -> Result<Value> {
    let before = context.now().await?;
    context.sleep(std::time::Duration::from_secs(3600)).await?;
    let after = context.now().await?;
    Ok(Value {
        value: (after.seconds - before.seconds).to_le_bytes().to_vec(),
    })
}

async fn empty(_: &Context, _: ()) -> Result<()> {
    Ok(())
}

async fn echo(_: &Context, input: Value) -> Result<Value> {
    Ok(input)
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
    match input.value.first() {
        Some(b'h') => Ok(Value {
            value: context.operation(HostInput { value: input.value }).await?.value,
        }),
        Some(b'p') => context.operation(PublicInput { value: input.value }).await,
        Some(b's') => context.operation(PrivateInput { value: input.value }).await,
        _ => context.operation(MissingInput { value: input.value }).await,
    }
}

async fn wait(context: &Context, input: Value) -> Result<Value> {
    context.operation(HostInput { value: input.value.clone() }).await?;
    context.cancelled().await;
    Ok(input)
}

/* === Tests === */
