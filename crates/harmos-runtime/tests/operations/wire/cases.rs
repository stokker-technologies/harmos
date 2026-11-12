/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports
use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{Failed, Owner};
use harmos_runtime::operations::{Backoff, FailureKind, Jitter, OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Message, Refusal};

/* === Constants === */

const ECHO_POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::OnTransient {
        total_attempts: 2,
        backoff: Backoff::Fixed {
            delay: Duration::from_millis(1),
            jitter: Jitter::None,
        },
    },
};

const NESTED_POLICY: OperationPolicy = OperationPolicy {
    retries: RetryPolicy::Never,
    ..ECHO_POLICY
};

const CODEC_POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};

/* === Macros === */

thread_local! {
    static DECODE_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}

// Each EchoDeclaration Request Carries One Text Value.
macro_rules! text {
    ($($ty:ident),* $(,)?) => {$(
        impl From<String> for $ty {
            fn from(value: String) -> Self {
                Self { value }
            }
        }

        impl From<&str> for $ty {
            fn from(value: &str) -> Self {
                Self {
                    value: value.to_owned(),
                }
            }
        }
    )*};
}

// A Text Codec With Scripted Panics And A Decode Hook.
macro_rules! scripted_codec {
    ($($ty:ident),* $(,)?) => {$(
        impl Message for $ty {
            const CODEC: &'static str = <Text as Message>::CODEC;
            fn encode(&self) -> Vec<u8> {
                assert_ne!(self.0.value, "encode_panic", "scripted codec panic");
                Message::encode(&self.0)
            }

            fn encoded_size(&self) -> usize {
                Message::encoded_size(&self.0)
            }

            fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
                <Text as Message>::decoding_memory(bytes)
            }

            fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
                if let Some(run) = DECODE_HOOK.with(|hook| hook.borrow_mut().take()) {
                    run();
                }
                let value = <Text as Message>::decode(bytes)?;
                assert_ne!(value.value, "decode_panic", "scripted codec panic");
                Ok(Self(value))
            }
        }



        impl harmos_runtime::MemorySize for $ty {
            fn memory_size(&self) -> usize {
                self.0.memory_size()
            }
        }

        impl harmos_runtime::schema::Described for $ty {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($ty));
            const FIELDS: &'static [harmos_runtime::schema::Field] = <Text as harmos_runtime::schema::Described>::FIELDS;
            const MARKER: &'static [u8] = &harmos_runtime::schema::declaration::encode::<
                { harmos_runtime::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
            >(Self::NAME, Self::FIELDS);
        }
    )*};
}

/* === Structs === */

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = String)]
#[derive(Clone)]
pub(super) struct EchoDeclaration {
    pub value: String,
}

pub(super) static ECHO_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<EchoDeclaration>("fixture.echo", ECHO_POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public);

pub(super) const ECHO_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<EchoDeclaration> =
    harmos_runtime::operations::OperationSpec::<EchoDeclaration>::new(&ECHO_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = String)]
#[derive(Clone, Debug)]
pub(super) struct PrivateEchoDeclaration {
    pub value: String,
}

pub(super) static PRIVATE_ECHO_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<PrivateEchoDeclaration>("fixture.protected_echo", ECHO_POLICY);

pub(super) const PRIVATE_ECHO_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<PrivateEchoDeclaration> =
    harmos_runtime::operations::OperationSpec::<PrivateEchoDeclaration>::new(&PRIVATE_ECHO_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = String)]
#[derive(Clone, Debug)]
pub(super) struct NestedDeclaration {
    pub value: String,
}

pub(super) static NESTED_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<NestedDeclaration>("fixture.nested_echo", NESTED_POLICY)
        .visibility(harmos_runtime::operations::Visibility::Public);

pub(super) const NESTED_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<NestedDeclaration> =
    harmos_runtime::operations::OperationSpec::<NestedDeclaration>::new(&NESTED_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = String)]
#[derive(Clone, Debug)]
pub(super) struct DetachedDeclaration {
    pub value: String,
}

pub(super) static DETACHED_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<DetachedDeclaration>("fixture.detached_echo", NESTED_POLICY);

pub(super) const DETACHED_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<DetachedDeclaration> =
    harmos_runtime::operations::OperationSpec::<DetachedDeclaration>::new(&DETACHED_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = String)]
#[derive(Clone, Debug)]
struct SameLaneDeclaration {
    pub value: String,
}

static SAME_LANE_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<SameLaneDeclaration>("fixture.same_lane_echo", NESTED_POLICY);

const SAME_LANE_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<SameLaneDeclaration> =
    harmos_runtime::operations::OperationSpec::<SameLaneDeclaration>::new(&SAME_LANE_DECLARATION_DESCRIPTION);

struct Joined<'a>(&'a std::sync::atomic::AtomicUsize);

#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = Payload)]
#[derive(Clone)]
struct Codec(Text);

static CODEC_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<Codec>("fixture.codec", CODEC_POLICY);

const CODEC_OPERATION: harmos_runtime::operations::OperationSpec<Codec> =
    harmos_runtime::operations::OperationSpec::<Codec>::new(&CODEC_DESCRIPTION);

pub(super) struct Payload(Text);

#[derive(harmos_runtime::Message, Clone, Default, Debug)]
pub(super) struct Text {
    pub value: String,
}

pub(super) struct Destination(pub &'static str);

/* === Implementations === */

impl From<String> for Text {
    fn from(value: String) -> Self {
        Self { value }
    }
}

impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Self { value: value.to_owned() }
    }
}

text!(EchoDeclaration, PrivateEchoDeclaration, NestedDeclaration, DetachedDeclaration, SameLaneDeclaration);

scripted_codec!(Payload, Codec);

async fn codec_handler(
    request: Codec, ctx: harmos_runtime::operations::FunctionContext<Codec>,
) -> Result<harmos_runtime::operations::Response<Payload>, Failed> {
    let resources = ctx.resource::<Owner>().expect("declared resource is registered");
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    if context.attempt() == 0 {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
    }
    let result: ::core::result::Result<Payload, Failed> = async move {
        resources.runs.fetch_add(1, Ordering::SeqCst);
        Ok(Payload(Text { value: request.0.value }))
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn codec_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        CODEC_OPERATION,
        const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        codec_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

pub(super) async fn echo_declaration_handler(
    request: EchoDeclaration, ctx: harmos_runtime::operations::FunctionContext<EchoDeclaration>,
) -> Result<harmos_runtime::operations::Response<String>, Failed> {
    let resources = ctx.resource::<Owner>().expect("declared resource is registered");
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    if context.attempt() == 0 {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
    }
    let result: ::core::result::Result<String, Failed> = async move {
        resources.runs.fetch_add(1, Ordering::SeqCst);
        resources.seen.lock().unwrap().push(context.id());
        Ok(format!("{}:{}", request.value, context.attempt() + 1))
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn echo_declaration_classification(error: &Failed) -> FailureKind {
    Owner::classify(error)
}

pub(super) fn echo_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        ECHO_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        echo_declaration_handler,
        echo_declaration_classification,
    )
}

pub(super) async fn private_echo_declaration_handler(
    request: PrivateEchoDeclaration, ctx: harmos_runtime::operations::FunctionContext<PrivateEchoDeclaration>,
) -> Result<harmos_runtime::operations::Response<String>, Failed> {
    let resources = ctx.resource::<Owner>().expect("declared resource is registered");
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    if context.attempt() == 0 {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
    }
    let result: ::core::result::Result<String, Failed> = async move {
        resources.runs.fetch_add(1, Ordering::SeqCst);
        resources.seen.lock().unwrap().push(context.id());
        if request.value == "block" {
            let _joined = Joined(&resources.wire_joined);
            resources.wire_started.notify_one();
            std::future::pending::<()>().await;
        }
        Ok(format!("{}:{}", request.value, context.attempt() + 1))
    }
    .await;
    result.map(::core::convert::Into::into)
}

pub(super) fn private_echo_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        PRIVATE_ECHO_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        private_echo_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl Drop for Joined<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

pub(super) async fn nested_declaration_handler(
    request: NestedDeclaration, ctx: harmos_runtime::operations::FunctionContext<NestedDeclaration>,
) -> Result<harmos_runtime::operations::Response<String>, Failed> {
    let resources = ctx.resource::<Owner>().expect("declared resource is registered");
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    if context.attempt() == 0 {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
    }
    let destination = ctx.resource::<Destination>().expect("declared resource is registered");
    let result: ::core::result::Result<String, Failed> = async move {
        let reply = context
            .call_encoded(&format!("{}.{}", destination.0, PRIVATE_ECHO_DECLARATION_OPERATION.entry().id), protobuf(&request))
            .await?;
        Ok(decode_protobuf::<String>(&reply))
    }
    .await;
    result.map(::core::convert::Into::into)
}

pub(super) fn nested_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        NESTED_DECLARATION_OPERATION,
        const {
            &[
                harmos_runtime::Resource::of::<Owner>(),
                harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>(),
                harmos_runtime::Resource::of::<Destination>(),
            ]
        },
        nested_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

async fn same_lane_declaration_handler(
    request: SameLaneDeclaration, ctx: harmos_runtime::operations::FunctionContext<SameLaneDeclaration>,
) -> Result<harmos_runtime::operations::Response<String>, Failed> {
    let resources = ctx.resource::<Owner>().expect("declared resource is registered");
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    if context.attempt() == 0 {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
    }
    let result: ::core::result::Result<String, Failed> = async move {
        let reply = context.call_encoded(&format!("host.{}", PRIVATE_ECHO_DECLARATION_OPERATION.entry().id), protobuf(&request)).await?;
        Ok(decode_protobuf::<String>(&reply))
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn same_lane_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        SAME_LANE_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        same_lane_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

pub(super) async fn detached_declaration_handler(
    _request: DetachedDeclaration, context: harmos_runtime::operations::FunctionContext<DetachedDeclaration>,
) -> Result<harmos_runtime::operations::Response<String>, Failed> {
    let resources = context.resource::<Owner>().expect("declared resource is registered");
    let host = context.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let result: ::core::result::Result<String, Failed> = async move {
        let started = resources.wire_started.notified();
        let _child =
            context.submit_encoded(&format!("host.{}", PRIVATE_ECHO_DECLARATION_OPERATION.entry().id), protobuf(&Text::from("block")))?;
        started.await;
        Ok("parent".to_owned())
    }
    .await;
    result.map(::core::convert::Into::into)
}

pub(super) fn detached_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        DETACHED_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        detached_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

/* === Private Functions === */

async fn runtime_with_owner(owner: Arc<Owner>) -> harmos_runtime::Runtime<crate::operations::support::Bank> {
    crate::operations::fixture::runtime_with(|setup| {
        setup.overrides(|setup| {
            setup.resource::<Owner>(owner).resource::<Destination>(Arc::new(Destination("host")));
        });
    })
    .await
}

#[tokio::test]
async fn malformed_payload_refused() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_owner(owner.clone()).await;
    runtime
        .operations
        .register_operation(echo_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    assert!(matches!(
        runtime.operations.submit_encoded(&format!("host.{}", ECHO_DECLARATION_OPERATION.entry().id), vec![0xff]),
        Err(Error::OperationPayload { .. })
    ));
    // The refused payload constructs nothing.
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn decode_panic_refused() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_owner(owner.clone()).await;
    runtime
        .operations
        .register_operation(codec_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    assert!(matches!(
        runtime.operations.submit_encoded(&format!("host.{}", CODEC_OPERATION.entry().id), protobuf(&Text::from("decode_panic"))),
        Err(Error::OperationPanicked { .. })
    ));
    // The panic stays inside submission and constructs nothing.
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn encode_panic_settles() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_owner(owner.clone()).await;
    runtime
        .operations
        .register_operation(codec_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    assert!(matches!(
        runtime.operations.call_encoded(&format!("host.{}", CODEC_OPERATION.entry().id), protobuf(&Text::from("encode_panic"))).await,
        Err(Error::OperationPanicked { .. })
    ));
    // The response panics after one completed run.
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 1);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn decode_races_replacement() {
    let old = Arc::new(Owner::default());
    let replacement = old.clone();
    let runtime = runtime_with_owner(old.clone()).await;
    runtime
        .operations
        .register_operation(codec_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let operations = runtime.operations.clone();
    DECODE_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
                    operations
                        .unregister(&harmos_runtime::QualifiedId::qualify("host", CODEC_OPERATION.entry().id).unwrap())
                        .await
                        .unwrap();
                    operations
                        .register_operation(
                            codec_operation().expect("representable operation"),
                            harmos_runtime::operations::Lane::default(),
                        )
                        .await
                        .unwrap();
                });
            })
            .join()
            .unwrap();
        }));
    });
    assert!(matches!(
        runtime.operations.submit_encoded(&format!("host.{}", CODEC_OPERATION.entry().id), protobuf(&Text::from("original"))),
        Err(Error::Closing { .. })
    ));
    assert_eq!(old.prepared.load(Ordering::SeqCst), 0);
    assert_eq!(replacement.prepared.load(Ordering::SeqCst), 0);
    let reply = runtime
        .operations
        .call_encoded(&format!("host.{}", CODEC_OPERATION.entry().id), protobuf(&Text::from("replacement")))
        .await
        .unwrap();
    assert_eq!(decode_protobuf::<Text>(&reply).value, "replacement");
    assert_eq!(replacement.prepared.load(Ordering::SeqCst), 1);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn children_refuse_held() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_owner(owner.clone()).await;
    runtime
        .operations
        .register_operation(private_echo_declaration_operation().expect("representable operation"), crate::support::SERIAL)
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(same_lane_declaration_operation().expect("representable operation"), crate::support::SERIAL)
        .await
        .unwrap();
    let error = runtime.operation(SameLaneDeclaration::from("child")).await.unwrap_err();
    let Error::OperationFailed { source, .. } = error else {
        panic!("nested refusal retained");
    };
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Nested(Error::OperationLaneCycle { .. }))));
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn success_joins_unawaited() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_owner(owner.clone()).await;
    runtime
        .operations
        .register_operation(
            private_echo_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        )
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(detached_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    assert_eq!(&*runtime.operation(DetachedDeclaration::from("ignored")).await.unwrap(), "parent");
    assert_eq!(owner.wire_joined.load(Ordering::SeqCst), 1);
    runtime
        .operations
        .unregister(&harmos_runtime::QualifiedId::qualify("host", PRIVATE_ECHO_DECLARATION_OPERATION.entry().id).unwrap())
        .await
        .unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn cancellation_joins_child() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_owner(owner.clone()).await;
    runtime
        .operations
        .register_operation(
            private_echo_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        )
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(nested_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let started = owner.wire_started.notified();
    let parent = runtime.operation(NestedDeclaration::from("block")).start().await.unwrap();
    started.await;
    parent.request_cancel();
    assert!(matches!(parent.await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
    assert_eq!(owner.wire_joined.load(Ordering::SeqCst), 1);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

fn protobuf<T: harmos_runtime::Portable>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    harmos_operations::execution::grpc::encode(value, &mut bytes).unwrap();
    bytes
}

fn decode_protobuf<T: harmos_runtime::Portable>(bytes: &[u8]) -> T {
    harmos_operations::execution::grpc::decode(harmos_operations::execution::grpc::DecodeInput::new(bytes)).unwrap()
}
