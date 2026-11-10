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
use harmos_protocol::v1;
use harmos_sdk_grpc::{Context, Message, Operation, OperationRequest, Plugin, Result, Visibility};

/* === Constants === */

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
        .map_err(|error| harmos_sdk_grpc::Error::Invalid(error.to_string()))?;
    let request = args.next().unwrap();
    let response = args.next().unwrap();
    let pool = prost_reflect::DescriptorPool::from_file_descriptor_set(types.clone()).unwrap();
    let request_tag = pool.get_message_by_name(&request).unwrap().get_field_by_name("value").unwrap().number();
    let response_tag = pool.get_message_by_name(&response).unwrap().get_field_by_name("value").unwrap().number();
    HOST.set((types, request, response, request_tag, response_tag)).unwrap();
    let plugin = if mode == "provider" {
        Plugin::new("provider", "1.0.0")
            .operation(Operation::new("fixture.public", echo).visibility(Visibility::Public))
            .operation(Operation::new("fixture.private", echo))
    } else if mode == "incompatible" {
        Plugin::new("incompatible", "1.0.0").operation(Operation::new("fixture.public", empty).visibility(Visibility::Public))
    } else if mode == "duplicate" {
        Plugin::new("duplicate", "1.0.0").operation(Operation::new("fixture.clock", clock)).operation(Operation::new("fixture.echo", echo))
    } else {
        Plugin::new("caller", "1.0.0")
            .requires::<HostInput>()
            .requires::<harmos_sdk_grpc::clock::Sleep>()
            .requires::<harmos_sdk_grpc::clock::Now>()
            .requires::<PublicInput>()
            .requires::<PrivateInput>()
            .requires::<MissingInput>()
            .operation(Operation::new("fixture.clock", clock))
            .operation(Operation::new("fixture.echo", echo))
            .operation(Operation::new("fixture.forward", forward))
            .operation(Operation::new("fixture.wait", wait).policy(v1::OperationPolicy {
                cancellation_timeout: Some(prost_types::Duration { seconds: 30, nanos: 0 }),
                ..Default::default()
            }))
    };
    plugin.remote().serve()
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
