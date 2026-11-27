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
use std::result;
use std::{
    env, fs, io,
    num::NonZeroUsize,
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

// Third Party Imports
use prost::Message;
use prost::{
    DecodeError,
    bytes::{Buf, BufMut},
    encoding::{DecodeContext, WireType},
};
use prost_reflect::{DescriptorPool, ReflectMessage};
use prost_types::{FileDescriptorSet, Timestamp};

// Local Imports
use harmos::plugin::HostOperation;
use harmos::plugin::TimeGetResponse;
use harmos::plugin::{Context, Contribution, Dependency, Descriptor, Observer, Operation, Plugin, Publisher, Stream, Window};
use harmos::{OperationRequest, Result, Visibility};
use harmos_protocol::host::{StatusGetRequest, StatusGetResponse};
use harmos_protocol::model::Classification;
use harmos_protocol::model::Failure;
use harmos_protocol::model::OperationPolicy;
use harmos_protocol::model::RetryPolicy;
use harmos_protocol::model::SettingDeclaration;
use harmos_protocol::model::SettingValue;
use harmos_protocol::model::SettingsDeclarations;
use harmos_protocol::model::setting_value;

/* === Constants === */

static DEVICE_ACTIVE: AtomicBool = AtomicBool::new(false);
static SCOPED_SAMPLES: AtomicU64 = AtomicU64::new(0);

static HOST: OnceLock<(FileDescriptorSet, String, String, u32, u32)> = OnceLock::new();

/* === Macros === */

macro_rules! host_message {
    ($type:ty, $index:tt) => {
        impl prost::Message for $type {
            fn encode_raw(&self, buf: &mut impl BufMut) {
                prost::encoding::bytes::encode(HOST.get().unwrap().$index, &self.value, buf);
            }
            fn merge_field(
                &mut self, tag: u32, wire: WireType, buf: &mut impl Buf, context: DecodeContext,
            ) -> result::Result<(), DecodeError> {
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

#[derive(harmos::Message)]
#[harmos(package = "fixture", file = "fixture.proto")]
struct Value {
    value: Vec<u8>,
}

#[derive(harmos::Message)]
#[harmos(package = "fixture", file = "fixture.proto")]
struct Reading {
    value: f64,
}

#[derive(harmos::Message)]
#[harmos(package = "fixture", file = "fixture.proto")]
struct OptionalReading {
    value: Option<f64>,
}

/// The Columns of `Reading`.
#[derive(harmos::Message)]
#[harmos(package = "fixture", file = "fixture.proto")]
struct Readings {
    value: Vec<f64>,
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

impl harmos::Message for HostInput {
    fn name() -> String {
        HOST.get().unwrap().1.clone()
    }
    fn types() -> FileDescriptorSet {
        let (types, request, ..) = HOST.get().unwrap();
        harmos_protocol::schema::message_types(types, request).expect("projected host request descriptors")
    }
}
impl harmos::Message for HostOutput {
    fn name() -> String {
        HOST.get().unwrap().2.clone()
    }
    fn types() -> FileDescriptorSet {
        let (types, _, response, ..) = HOST.get().unwrap();
        harmos_protocol::schema::message_types(types, response).expect("projected host response descriptors")
    }
}
#[derive(harmos::Message)]
#[harmos(package = "harmos.host.v1", file = "harmos/host/v1/time.proto", name = "TimeGetRequest")]
struct HostControl {}
impl OperationRequest for HostControl {
    type Response = TimeGetResponse;
    const ID: &'static str = "fixture.control";
}

// Adapt the Actual Client-Only Wire Types for a Deliberately Refused Plugin Call.
#[derive(Debug, Default)]
struct CallerProbe<T>(T);

impl<T: prost::Message + Default> prost::Message for CallerProbe<T> {
    fn encode_raw(&self, buf: &mut impl BufMut) {
        self.0.encode_raw(buf);
    }
    fn merge_field(&mut self, tag: u32, wire: WireType, buf: &mut impl Buf, context: DecodeContext) -> result::Result<(), DecodeError> {
        self.0.merge_field(tag, wire, buf, context)
    }
    fn encoded_len(&self) -> usize {
        self.0.encoded_len()
    }
    fn clear(&mut self) {
        self.0.clear();
    }
}

impl<T: ReflectMessage + Default + 'static> harmos::Message for CallerProbe<T> {
    fn name() -> String {
        T::default().descriptor().full_name().into()
    }
    fn types() -> FileDescriptorSet {
        let descriptors = T::default().descriptor().parent_pool().encode_to_vec();
        let types = FileDescriptorSet::decode(descriptors.as_slice()).expect("generated caller probe descriptors");
        let name = T::default().descriptor().full_name().to_owned();
        harmos_protocol::schema::message_types(&types, &name).expect("projected caller probe descriptors")
    }
}

impl OperationRequest for CallerProbe<StatusGetRequest> {
    type Response = CallerProbe<StatusGetResponse>;
    const ID: &'static str = "harmos.host.v1.Status/Get";
}

#[derive(harmos::Message)]
#[harmos(package = "harmos.host.v1", file = "harmos/host/v1/time.proto", name = "TimeGetRequest")]
struct ApplicationContext {}
impl OperationRequest for ApplicationContext {
    type Response = TimeGetResponse;
    const ID: &'static str = "example.host.v1.Prompts/Create";
}

#[derive(harmos::Message)]
#[harmos(package = "harmos.host.v1", file = "harmos/host/v1/time.proto", name = "TimeGetRequest")]
struct ApplicationClient {}
impl OperationRequest for ApplicationClient {
    type Response = TimeGetResponse;
    const ID: &'static str = "example.host.v1.Steps/Execute";
}

#[derive(harmos::Message)]
#[harmos(package = "harmos.host.v1", file = "harmos/host/v1/time.proto", name = "TimeGetRequest")]
struct ApplicationInstance {}
impl OperationRequest for ApplicationInstance {
    type Response = TimeGetResponse;
    const ID: &'static str = "example.plugin.v1.Control/Execute";
}

impl OperationRequest for HostInput {
    type Response = HostOutput;
    const ID: &'static str = "fixture.host";
}
impl harmos::Message for PublicInput {
    fn name() -> String {
        Value::name()
    }
    fn types() -> FileDescriptorSet {
        Value::types()
    }
}
impl OperationRequest for PublicInput {
    type Response = Value;
    const ID: &'static str = "fixture.plugin.v1.Steps/Execute";
}
impl harmos::Message for ProtectedInput {
    fn name() -> String {
        Value::name()
    }
    fn types() -> FileDescriptorSet {
        Value::types()
    }
}
impl OperationRequest for ProtectedInput {
    type Response = Value;
    const ID: &'static str = "fixture.plugin.v1.Lifecycle/Initialize";
}
impl harmos::Message for MissingInput {
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
impl harmos::Message for PeerInput {
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

#[derive(prost::Message)]
struct PrivateInput {
    #[prost(bytes = "vec", tag = "1")]
    value: Vec<u8>,
}
impl harmos::Message for PrivateInput {
    fn name() -> String {
        Value::name()
    }
    fn types() -> FileDescriptorSet {
        Value::types()
    }
}
impl OperationRequest for PrivateInput {
    type Response = Value;
    const ID: &'static str = "fixture.private";
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
    let plugin = if mode == "optional-window" {
        let stream = Stream::<OptionalReading>::new("fixture.optional");
        let observer = Observer::new(&stream, async |context: &Context| {
            let value = match context.sequence() % 3 {
                1 => None,
                2 => Some(0.0),
                _ => Some(2.5),
            };
            Ok(OptionalReading { value })
        })
        .every(prost_types::Duration { seconds: 1, nanos: 0 });
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(stream)?;
                ctx.register(observer)?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "scoped-sleep" {
        let stream = Stream::<Value>::new("fixture.scoped_samples");
        let observer = Observer::new(&stream, scoped_sleep_observe).every(prost_types::Duration { seconds: 1, nanos: 0 });
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(HostOperation::TimeGet)?;
                ctx.register(HostOperation::TimeSleep)?;
                ctx.register(stream)?;
                ctx.register(observer)?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "counted-sleep" {
        let stream = Stream::<Value>::new("fixture.counted_samples");
        let observer = Observer::new(&stream, counted_observe).every(prost_types::Duration { seconds: 1, nanos: 0 });
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(HostOperation::TimeSleep)?;
                ctx.register(stream)?;
                ctx.register(observer)?;
                ctx.register(Operation::new("fixture.count_after", count_after).visibility(Visibility::Protected))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "scoped-publications" {
        let stream = Stream::<Value>::new("fixture.scoped_samples");
        let observer = Observer::new(&stream, scoped_observe).every(prost_types::Duration { seconds: 1, nanos: 0 });
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(HostOperation::TimeGet)?;
                ctx.register(HostOperation::TimeSleep)?;
                ctx.register(stream)?;
                ctx.register(observer)?;
                ctx.register(Operation::new("fixture.clock", scoped_clock).visibility(Visibility::Protected))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "windows" {
        // One-Second Samples: Host-Length Windows on One Stream, a Plugin Setting's Length on the Other.
        let counts = Stream::<Value>::new("fixture.counts");
        let counted = Observer::new(&counts, observe).every(prost_types::Duration { seconds: 1, nanos: 0 });
        let overridden = Stream::<Value>::new("fixture.override");
        let override_observer =
            Observer::new(&overridden, observe).every(prost_types::Duration { seconds: 1, nanos: 0 }).window_setting("window");
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(duration_setting("window", "Window", 3, 0))?;
                ctx.register(counts)?;
                ctx.register(overridden)?;
                ctx.register(counted)?;
                ctx.register(override_observer)?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "split" {
        // Two-Second Windows of One-Microsecond Samples Outgrow One harmos::Message.
        let stream = Stream::<Reading>::new("fixture.split");
        let observer =
            Observer::window(&stream, split_observe).every(prost_types::Duration { seconds: 0, nanos: 1_000 }).window_setting("window");
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(duration_setting("window", "Window", 2, 0))?;
                ctx.register(stream)?;
                ctx.register(observer)?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "bulky" {
        // One-Mebibyte Samples Every Second: One Long Window Outgrows the Shared Publication Pool.
        let stream = Stream::<Value>::new("fixture.bulky");
        let observer = Observer::new(&stream, async |_: &Context| Ok(Value { value: vec![0; 1 << 20] }))
            .every(prost_types::Duration { seconds: 1, nanos: 0 });
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(stream)?;
                ctx.register(observer)?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "failing" {
        // Every Window Fails in the Plugin's Own Observer.
        let stream = Stream::<Value>::new("fixture.failing");
        let observer =
            Observer::new(&stream, async |_: &Context| -> Result<Value> { Err(harmos::Error::Invalid("Sensor Offline".into())) })
                .every(prost_types::Duration { seconds: 1, nanos: 0 });
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(stream)?;
                ctx.register(observer)?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "dense" {
        // One-Microsecond Readings Without a Window Setting: the Host's Window Length Alone Sizes Each Window.
        let stream = Stream::<Reading>::new("fixture.dense");
        let observer = Observer::window(&stream, split_observe).every(prost_types::Duration { seconds: 0, nanos: 1_000 });
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(stream)?;
                ctx.register(observer)?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "interval" {
        let stream = Stream::<Value>::new("fixture.interval");
        let observer = Observer::new(&stream, observe).every(prost_types::Duration { seconds: 60, nanos: 0 }).interval_setting("interval");
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(SettingsDeclarations {
                    values: vec![SettingDeclaration {
                        name: "interval".into(),
                        title: "Interval".into(),
                        description: String::new(),
                        default: Some(SettingValue {
                            value: Some(setting_value::Value::Duration(prost_types::Duration { seconds: 1, nanos: 0 })),
                        }),
                        minimum: None,
                        maximum: None,
                        choices: Vec::new(),
                        required: false,
                    }],
                })?;
                ctx.register(stream)?;
                ctx.register(observer)?;
                ctx.register(Operation::new("fixture.echo", echo).visibility(Visibility::Protected))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "error-kinds" {
        Plugin::new("fixture", "1.0.0")
            .failure_mapper(|error| {
                error
                    .downcast_ref::<io::Error>()
                    .filter(|error| error.to_string() == "Source Unavailable")
                    .map(|_| harmos::Error::failure(Classification::Refused, "source_missing", "Source Unavailable", "Select a Source"))
            })
            .register(|ctx| {
                ctx.register(
                    Operation::new("fixture.private", async |_: &Context, request: Value| -> Result<Value> {
                        let classification = match request.value.as_slice() {
                            b"transient" => Classification::Transient,
                            b"refused" => Classification::Refused,
                            b"invalid" => Classification::Invalid,
                            b"cancelled" => Classification::Cancelled,
                            b"mapped" => return Err(harmos::Error::other(io::Error::other("Source Unavailable"))),
                            b"unmapped" => return Err(harmos::Error::other(io::Error::other("Unmapped Failure"))),
                            _ => Classification::Permanent,
                        };
                        Err(harmos::Error::failure(classification, "fixture_failure", "Apply Failed", "Choose Another Input"))
                    })
                    .visibility(Visibility::Private)
                    .replay_safe()
                    .policy(OperationPolicy {
                        retries: Some(RetryPolicy {
                            total_attempts: 3,
                            initial: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                )?;
                ctx.register(
                    Operation::new("fixture.error_report", async |context: &Context, request: Value| {
                        let error = match context.operation(PrivateInput { value: request.value }).await {
                            Err(error) => error,
                            Ok(_) => panic!("Expected an Operation Failure"),
                        };
                        let retry = error.retry();
                        let value = serde_json::json!({
                            "kind": error.kind(), "code": error.code(), "message": error.message(), "action": error.action(),
                            "operation": error.operation(), "retry": {"retryable": retry.retryable, "attempts": retry.attempts,
                            "max_attempts": retry.max_attempts, "next_after_s": retry.next_after_s},
                        });
                        Ok(Value {
                            value: serde_json::to_vec(&value).unwrap(),
                        })
                    })
                    .visibility(Visibility::Protected),
                )?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "captures" {
        let stream = Stream::<Value>::new("fixture.captures");
        let observer = Observer::new(&stream, async |context: &Context| {
            let mut value = b"captures:".to_vec();
            value.extend(serde_json::to_vec(&context.capturing_executions).unwrap());
            let response = context.operation(HostInput { value }).await?;
            Ok(Value { value: response.value })
        })
        .every(prost_types::Duration { seconds: 1, nanos: 0 });
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(Dependency::<HostInput>::new())?;
                ctx.register(stream)?;
                ctx.register(observer)?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "private" {
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(Operation::new("fixture.own", call_private).visibility(Visibility::Protected))?;
                ctx.register(Operation::new("fixture.private", retry).visibility(Visibility::Private).replay_safe().policy(
                    OperationPolicy {
                        retries: Some(RetryPolicy {
                            total_attempts: 3,
                            initial: Some(prost_types::Duration { seconds: 2, nanos: 0 }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "private-peer" {
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(Dependency::<PrivateInput>::new())?;
                ctx.register(Operation::new("fixture.private_peer", call_private).visibility(Visibility::Protected))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "retry" {
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(Operation::new("fixture.retry", retry).visibility(Visibility::Protected).replay_safe().policy(
                    OperationPolicy {
                        retries: Some(RetryPolicy {
                            total_attempts: 3,
                            initial: Some(prost_types::Duration { seconds: 2, nanos: 0 }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "deadline" {
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(HostOperation::TimeGet)?;
                ctx.register(HostOperation::TimeSleep)?;
                ctx.register(Operation::new("fixture.clock", clock).visibility(Visibility::Protected).policy(OperationPolicy {
                    timeout: Some(prost_types::Duration { seconds: 3, nanos: 0 }),
                    attempt_timeout: Some(prost_types::Duration { seconds: 2, nanos: 0 }),
                    ..Default::default()
                }))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "twins" {
        let left = Stream::<Value>::new("fixture.left");
        let right = Stream::<Value>::new("fixture.right");
        let left_observer = Observer::new(&left, observe).every(prost_types::Duration { seconds: 1, nanos: 0 });
        let right_observer = Observer::new(&right, observe).every(prost_types::Duration { seconds: 2, nanos: 0 });
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(left)?;
                ctx.register(right)?;
                ctx.register(left_observer)?;
                ctx.register(right_observer)?;
                ctx.register(Operation::new("fixture.echo", echo).visibility(Visibility::Protected))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "device" {
        let stream = Stream::<Value>::new("fixture.device").publish_on(device);
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(stream)?;
                ctx.register(Operation::new("fixture.echo", echo).visibility(Visibility::Protected))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
            .on_stop(|| {
                if DEVICE_ACTIVE.load(Ordering::SeqCst) {
                    return Err(harmos::Error::Invalid("Device Demand Still Active".into()));
                }
                Ok(())
            })
    } else if mode == "scoped" {
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(Dependency::<HostInput>::new())?;
                ctx.register(Dependency::<PublicInput>::new())?;
                ctx.register(Operation::new("fixture.scoped", scoped).visibility(Visibility::Protected))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "scoped-provider" {
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(Operation::new("fixture.plugin.v1.Steps/Execute", scope_identity).visibility(Visibility::Public))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "values" {
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(HostOperation::PluginsSettingsList)?;
                ctx.register(Descriptor::<TimeGetResponse>::new())?;
                ctx.register(Contribution(SettingsDeclarations {
                    values: vec![SettingDeclaration {
                        name: "reading".into(),
                        title: "Reading".into(),
                        description: String::new(),
                        default: Some(SettingValue {
                            value: Some(setting_value::Value::Integer(0)),
                        }),
                        minimum: None,
                        maximum: None,
                        choices: Vec::new(),
                        required: false,
                    }],
                }))?;
                ctx.register(Operation::new("fixture.value", setting_value).visibility(Visibility::Protected))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "watch" {
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(HostOperation::InvocationsWatch)?;
                ctx.register(Operation::new("fixture.watch", watch_invocation).visibility(Visibility::Protected))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "provider" {
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(Operation::new("fixture.step.0", echo).visibility(Visibility::Public))?;
                ctx.register(Dependency::<HostInput>::new())?;
                ctx.register(Dependency::<ProtectedInput>::new())?;
                ctx.register(
                    Operation::new("fixture.self", async |context: &Context, input: Value| {
                        context.operation(ProtectedInput { value: input.value }).await
                    })
                    .visibility(Visibility::Protected),
                )?;
                ctx.register(Operation::new("fixture.peer", peer).visibility(Visibility::Public).policy(OperationPolicy {
                    cancellation_timeout: Some(prost_types::Duration { seconds: 30, nanos: 0 }),
                    ..Default::default()
                }))?;
                ctx.register(Operation::new("fixture.plugin.v1.Steps/Execute", echo).visibility(Visibility::Public))?;
                ctx.register(Operation::new("fixture.plugin.v1.Lifecycle/Initialize", echo).visibility(Visibility::Protected))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "incompatible" {
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(
                    Operation::new("fixture.plugin.v1.Steps/Execute", async |_: &Context, _: ()| Ok(())).visibility(Visibility::Public),
                )?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else if mode == "duplicate" {
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(Operation::new("fixture.echo", echo).visibility(Visibility::Protected))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    } else {
        Plugin::new("fixture", "1.0.0")
            .register(|ctx| {
                ctx.register(Dependency::<HostInput>::new())?;
                ctx.register(Dependency::<HostControl>::new())?;
                ctx.register(Dependency::<CallerProbe<StatusGetRequest>>::new())?;
                ctx.register(Dependency::<ApplicationContext>::new())?;
                ctx.register(Dependency::<ApplicationClient>::new())?;
                ctx.register(Dependency::<ApplicationInstance>::new())?;
                ctx.register(HostOperation::TimeSleep)?;
                ctx.register(HostOperation::TimeGet)?;
                ctx.register(Dependency::<PublicInput>::new())?;
                ctx.register(Dependency::<PeerInput>::new())?;
                ctx.register(Operation::new("fixture.peer_forward", peer_forward).visibility(Visibility::Protected).policy(
                    OperationPolicy {
                        cancellation_timeout: Some(prost_types::Duration { seconds: 30, nanos: 0 }),
                        ..Default::default()
                    },
                ))?;
                ctx.register(Dependency::<ProtectedInput>::new())?;
                ctx.register(Dependency::<MissingInput>::new())?;
                ctx.register(Operation::new("fixture.clock", clock).visibility(Visibility::Protected))?;
                ctx.register(Operation::new("fixture.echo", echo).visibility(Visibility::Protected))?;
                ctx.register(Operation::new("fixture.forward", forward).visibility(Visibility::Protected))?;
                ctx.register(Operation::new("fixture.failure", failure).visibility(Visibility::Protected))?;
                ctx.register(Operation::new("fixture.wait", wait).visibility(Visibility::Protected).policy(OperationPolicy {
                    cancellation_timeout: Some(prost_types::Duration { seconds: 30, nanos: 0 }),
                    ..Default::default()
                }))?;
                Ok(())
            })
            .expect("Register Plugin Declarations")
    };
    if args.next().as_deref() == Some("--managed") {
        plugin.run()
    } else {
        plugin.remote().run()
    }
}

async fn observe(context: &Context) -> Result<Value> {
    Ok(Value {
        value: context.sequence().to_le_bytes().to_vec(),
    })
}

/// Each Sample's Value Is Its Sequence.
async fn split_observe(_: &Context, window: &Window) -> Result<Readings> {
    Ok(Readings {
        value: (0..window.count).map(|index| (window.sequence + index) as f64).collect(),
    })
}

fn duration_setting(name: &str, title: &str, seconds: i64, nanos: i32) -> SettingsDeclarations {
    SettingsDeclarations {
        values: vec![SettingDeclaration {
            name: name.into(),
            title: title.into(),
            description: String::new(),
            default: Some(SettingValue {
                value: Some(setting_value::Value::Duration(prost_types::Duration { seconds, nanos })),
            }),
            minimum: None,
            maximum: None,
            choices: Vec::new(),
            required: false,
        }],
    }
}

async fn scoped_observe(context: &Context) -> Result<Value> {
    // The Attachment's Unscoped Demand Remains Usable Alongside Scoped Demands.
    let scope = context.scope().unwrap_or("unscoped");
    let now = context.now().await?;
    Ok(Value {
        value: format!("{scope}:{}", now.seconds).into_bytes(),
    })
}

/// Count Each Scoped Sample This Plugin Observes.
async fn counted_observe(context: &Context) -> Result<Value> {
    let count = if context.scope().is_ok() {
        SCOPED_SAMPLES.fetch_add(1, Ordering::SeqCst) + 1
    } else {
        0
    };
    Ok(Value { value: count.to_le_bytes().to_vec() })
}

/// Sleep the Requested Seconds Once, Then Report the Scoped Samples Observed So Far.
async fn count_after(context: &Context, request: Value) -> Result<Value> {
    let seconds =
        u64::from_le_bytes(request.value.as_slice().try_into().map_err(|_| harmos::Error::Invalid("Count After: eight bytes".into()))?);
    context.sleep(Duration::from_secs(seconds)).await?;
    Ok(Value {
        value: SCOPED_SAMPLES.load(Ordering::SeqCst).to_le_bytes().to_vec(),
    })
}

async fn scoped_sleep_observe(context: &Context) -> Result<Value> {
    context.sleep(Duration::from_secs(1)).await?;
    let scope = context.scope().unwrap_or("unscoped");
    let now = context.now().await?;
    Ok(Value {
        value: format!("{scope}:{}:{}", context.sequence(), now.seconds).into_bytes(),
    })
}

async fn device(context: &Context, publisher: Publisher<Value>) -> Result<()> {
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

async fn scoped_clock(context: &Context, _: Value) -> Result<Value> {
    let before = context.now().await?;
    context.sleep(Duration::from_secs(3)).await?;
    let after = context.now().await?;
    Ok(Value {
        value: (after.seconds - before.seconds).to_le_bytes().to_vec(),
    })
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
    let host = context.operation(HostInput { value: b"scope".to_vec() }).await?;
    let plugin = context.operation(PublicInput { value: Vec::new() }).await?;
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

async fn call_private(context: &Context, request: Value) -> Result<Value> {
    context.operation(PrivateInput { value: request.value }).await
}

async fn retry(context: &Context, input: Value) -> Result<Value> {
    if context.attempt < 3 {
        return Err(harmos::Error::failure(Classification::Transient, "retry", "Retry", "Try Again"));
    }
    Ok(input)
}

async fn failure(_: &Context, _: Value) -> Result<Value> {
    Err(Failure {
        classification: Classification::Permanent as i32,
        code: "fixture_failure".into(),
        message: "Fixture Failed".into(),
        action: "Use Another Input".into(),
        details: [("fixture".into(), "value".into())].into(),
        diagnostics: "Fixture Diagnostics".into(),
        cause: Some(Box::new(Failure {
            classification: Classification::Transient as i32,
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
        context.operation(CallerProbe(StatusGetRequest {})).await?;
        return Ok(input);
    }
    if input.value == b"control" {
        context.operation(HostControl {}).await?;
        return Ok(input);
    }
    if input.value == b"application-context" {
        let response = context.operation(ApplicationContext {}).await?;
        return Ok(Value {
            value: response.now.unwrap().seconds.to_le_bytes().to_vec(),
        });
    }
    if input.value == b"application-client" {
        context.operation(ApplicationClient {}).await?;
        return Ok(input);
    }
    if input.value == b"application-instance" {
        context.operation(ApplicationInstance {}).await?;
        return Ok(input);
    }
    match input.value.first() {
        Some(b'h') => Ok(Value {
            value: context.operation(HostInput { value: input.value }).await?.value,
        }),
        Some(b'p') => context.operation(PublicInput { value: input.value }).await,
        Some(b's') => context.operation(ProtectedInput { value: input.value }).await,
        _ => context.operation(MissingInput { value: input.value }).await,
    }
}

async fn wait(context: &Context, input: Value) -> Result<Value> {
    context.operation(HostInput { value: input.value.clone() }).await?;
    context.cancelled().await;
    Ok(input)
}

/* === Tests === */

async fn watch_invocation(context: &Context, request: Value) -> Result<Value> {
    let instance =
        u64::from_le_bytes(request.value.as_slice().try_into().map_err(|_| harmos::Error::Invalid("Watch: Instance Required".into()))?);
    let mut watch = context.host().invocations.watch(instance, NonZeroUsize::new(4096).unwrap()).await?;
    let mut phases = Vec::new();
    while let Some(state) = watch.next().await? {
        phases.extend(state.phase.as_bytes());
        phases.push(b'|');
    }
    Ok(Value { value: phases })
}

async fn setting_value(context: &Context, request: Value) -> Result<Value> {
    let name = String::from_utf8(request.value).map_err(|error| harmos::Error::Invalid(format!("List Settings: {error}")))?;
    let response = context.settings().list().await?;
    let value = response
        .value
        .and_then(|settings| settings.values.get(&name).cloned())
        .ok_or_else(|| harmos::Error::Invalid(format!("List Settings: Missing {name}")))?;
    Ok(Value { value: value.encode_to_vec() })
}

async fn peer_forward(context: &Context, input: Value) -> Result<Value> {
    match context.operation(PeerInput { value: input.value.clone() }).await {
        Err(harmos::Error::Failure(failure)) if failure.classification() == Classification::Cancelled => Ok(input),
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
