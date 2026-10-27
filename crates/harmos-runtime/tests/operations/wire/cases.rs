/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{Failed, Lanes, Owner, runtime};
use harmos_runtime::operations::{Backoff, CacheKey, FailureKind, Handler, Jitter, OperationDeclaration, OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Message, Refusal};

/* === Constants === */

const ECHO_POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::OnTransient {
        max_retries: 1,
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

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::operation(id = "fixture_echo",
    response = String,
    visibility = Shared,
    policy = ECHO_POLICY,
    key = |request: &EchoDeclaration| (request.value == "retry")
        .then(|| CacheKey::new(request.value.as_bytes().to_vec()).unwrap()),
)]
pub(super) struct EchoDeclaration {
    pub value: String,
}

#[harmos_runtime::operation(id = "fixture_private_echo",
    response = String,
    policy = ECHO_POLICY
)]
#[derive(Debug)]
pub(super) struct PrivateEchoDeclaration {
    pub value: String,
}

#[harmos_runtime::operation(id = "fixture_nested_echo",
    response = String,
    visibility = Shared,
    policy = NESTED_POLICY
)]
#[derive(Debug)]
pub(super) struct NestedDeclaration {
    pub value: String,
}

#[harmos_runtime::operation(id = "fixture_detached_echo",
    response = String,
    policy = NESTED_POLICY
)]
#[derive(Debug)]
pub(super) struct DetachedDeclaration {
    pub value: String,
}

#[harmos_runtime::operation(id = "fixture_same_lane_echo",
    response = String,
    policy = NESTED_POLICY
)]
#[derive(Debug)]
struct SameLaneDeclaration {
    pub value: String,
}

struct Joined<'a>(&'a std::sync::atomic::AtomicUsize);

#[harmos_runtime::operation(codec = manual, id = "fixture_codec",  response = Payload, policy = CODEC_POLICY)]
struct Codec(Text);

pub(super) struct Payload(Text);

#[harmos_runtime::message]
#[derive(Debug)]
pub(super) struct Text {
    pub value: String,
}

pub(super) struct State {
    attempts: usize,
}

/* === Enums === */

#[harmos_runtime::catalog]
pub(super) enum Leaves {
    EchoDeclaration(EchoDeclaration),
    PrivateEchoDeclaration(PrivateEchoDeclaration),
    NestedDeclaration(NestedDeclaration),
    DetachedDeclaration(DetachedDeclaration),
}

#[harmos_runtime::catalog]
pub(super) enum Group {
    #[nested]
    Leaves(Leaves),
}

#[harmos_runtime::catalog]
pub(super) enum Operations {
    #[nested]
    Empty(()),

    #[nested]
    Group(Group),
}

/* === Traits === */

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

struct Coding {
    resources: Arc<Owner>,
}

impl Handler for Coding {
    type StreamEndpoints = ();
    type Request = Codec;
    type Response = <Codec as harmos_runtime::OperationDeclaration>::Response;
    type Error = Failed;

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Failed> {
        let resources = context.resource::<Owner>().expect("fixture registers its dependency");
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { resources })
    }

    async fn run(
        &mut self, _context: harmos_runtime::OperationContext, request: &mut Codec, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<Payload>, Failed> {
        let host = _context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let _context = host.borrow();
        let result: ::core::result::Result<Payload, Self::Error> = async move {
            self.resources.runs.fetch_add(1, Ordering::SeqCst);
            Ok(Payload(Text {
                value: std::mem::take(&mut request.0.value),
            }))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

pub(super) struct Echoing {
    resources: Arc<Owner>,
    state: State,
}

impl Handler for Echoing {
    type StreamEndpoints = ();
    type Request = EchoDeclaration;
    type Response = String;
    type Error = Failed;

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Failed> {
        let resources = context.resource::<Owner>().expect("fixture registers its dependency");
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        let state = State { attempts: 0 };
        Ok(Self { resources, state })
    }

    async fn run(
        &mut self, context: harmos_runtime::OperationContext, value: &mut EchoDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<String>, Failed> {
        let host = context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<String, Self::Error> = async move {
            self.resources.runs.fetch_add(1, Ordering::SeqCst);
            self.resources.seen.lock().unwrap().push(context.id());
            self.state.attempts += 1;
            Ok(format!("{}:{}", value.value, self.state.attempts))
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    fn classify(error: &Failed) -> FailureKind {
        Owner::classify(error)
    }
}

pub(super) struct PrivateEchoing {
    resources: Arc<Owner>,
    state: State,
}

impl Handler for PrivateEchoing {
    type StreamEndpoints = ();
    type Request = PrivateEchoDeclaration;
    type Response = String;
    type Error = Failed;

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Failed> {
        let resources = context.resource::<Owner>().expect("fixture registers its dependency");
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        let state = State { attempts: 0 };
        Ok(Self { resources, state })
    }

    async fn run(
        &mut self, context: harmos_runtime::OperationContext, value: &mut PrivateEchoDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<String>, Failed> {
        let host = context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<String, Self::Error> = async move {
            self.resources.runs.fetch_add(1, Ordering::SeqCst);
            self.resources.seen.lock().unwrap().push(context.id());
            self.state.attempts += 1;
            if value.value == "block" {
                let _joined = Joined(&self.resources.wire_joined);
                self.resources.wire_started.notify_one();
                std::future::pending::<()>().await;
            }
            Ok(format!("{}:{}", value.value, self.state.attempts))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Drop for Joined<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

pub(super) struct Nesting<const PLUGIN: bool = false> {
    _resources: Arc<Owner>,
}

impl<const PLUGIN: bool> Handler for Nesting<PLUGIN> {
    type StreamEndpoints = ();
    type Request = NestedDeclaration;
    type Response = String;
    type Error = Failed;

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Failed> {
        let resources = context.resource::<Owner>().expect("fixture registers its dependency");
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { _resources: resources })
    }

    async fn run(
        &mut self, context: harmos_runtime::OperationContext, request: &mut NestedDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<String>, Failed> {
        let host = context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<String, Self::Error> = async move {
            let reply = context
                .call_encoded(
                    &format!("{}.{}", if PLUGIN { "fixture" } else { "host" }, PrivateEchoDeclaration::ID),
                    Message::encode(request),
                )
                .await?;
            Ok(String::decode(&reply).unwrap())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

struct SameLaneCall {
    _resources: Arc<Owner>,
}

impl Handler for SameLaneCall {
    type StreamEndpoints = ();
    type Request = SameLaneDeclaration;
    type Response = String;
    type Error = Failed;

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Failed> {
        let resources = context.resource::<Owner>().expect("fixture registers its dependency");
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { _resources: resources })
    }

    async fn run(
        &mut self, context: harmos_runtime::OperationContext, request: &mut SameLaneDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<String>, Failed> {
        let host = context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<String, Self::Error> = async move {
            let reply = context.call_encoded(&format!("host.{}", PrivateEchoDeclaration::ID), Message::encode(request)).await?;
            Ok(String::decode(&reply).unwrap())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

pub(super) struct Detaching {
    resources: Arc<Owner>,
}

impl Handler for Detaching {
    type StreamEndpoints = ();
    type Request = DetachedDeclaration;
    type Response = String;
    type Error = Failed;

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Failed> {
        let resources = context.resource::<Owner>().expect("fixture registers its dependency");
        Ok(Self { resources })
    }

    async fn run(
        &mut self, context: harmos_runtime::OperationContext, _request: &mut DetachedDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<String>, Failed> {
        let host = context.resource::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<String, Self::Error> = async move {
            let started = self.resources.wire_started.notified();
            let _child = context.submit_encoded(&format!("host.{}", PrivateEchoDeclaration::ID), Text::from("block").encode())?;
            started.await;
            Ok("parent".to_owned())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn malformed_payload_refused() {
    let runtime = runtime().await;
    let owner = Arc::new(Owner::default());
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(owner.clone()).unwrap();
                dependencies
            })
            .implementation::<EchoDeclaration, Echoing>()
            .implementation::<PrivateEchoDeclaration, PrivateEchoing>()
            .implementation::<NestedDeclaration, Nesting>()
            .implementation::<DetachedDeclaration, Detaching>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    assert!(matches!(
        runtime.operations.submit_encoded(&format!("host.{}", EchoDeclaration::ID), vec![0xff]),
        Err(Error::OperationPayload { .. })
    ));
    // The refused payload constructs nothing.
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn decode_panic_refused() {
    let runtime = runtime().await;
    let owner = Arc::new(Owner::default());
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(owner.clone()).unwrap();
                dependencies
            })
            .implementation::<Codec, Coding>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    assert!(matches!(
        runtime.operations.submit_encoded(&format!("host.{}", Codec::ID), Text::from("decode_panic").encode()),
        Err(Error::OperationPanicked { .. })
    ));
    // The panic stays inside submission and constructs nothing.
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn encode_panic_settles() {
    let runtime = runtime().await;
    let owner = Arc::new(Owner::default());
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(owner.clone()).unwrap();
                dependencies
            })
            .implementation::<Codec, Coding>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    assert!(matches!(
        runtime.operations.call_encoded(&format!("host.{}", Codec::ID), Text::from("encode_panic").encode()).await,
        Err(Error::OperationPanicked { .. })
    ));
    // The response panics after one completed run.
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn decode_races_replacement() {
    let runtime = runtime().await;
    let old = Arc::new(Owner::default());
    let replacement = Arc::new(Owner::default());
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(old.clone()).unwrap();
                dependencies
            })
            .implementation::<Codec, Coding>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    let operations = runtime.operations.clone();
    let new_owner = replacement.clone();
    DECODE_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
                    operations.unregister(&harmos_runtime::QualifiedId::qualify("host", Codec::ID).unwrap()).await.unwrap();
                    operations
                        .register(harmos_runtime::RegistrationExt::lane(
                            harmos_runtime::RegistrationBatch::new({
                                let mut dependencies = harmos_runtime::Dependencies::new();
                                dependencies.insert(new_owner).unwrap();
                                dependencies
                            })
                            .implementation::<Codec, Coding>(),
                            Lanes::Open,
                        ))
                        .await
                        .unwrap();
                });
            })
            .join()
            .unwrap();
        }));
    });
    assert!(matches!(
        runtime.operations.submit_encoded(&format!("host.{}", Codec::ID), Text::from("original").encode()),
        Err(Error::Closing { .. })
    ));
    assert_eq!(old.prepared.load(Ordering::SeqCst), 0);
    assert_eq!(replacement.prepared.load(Ordering::SeqCst), 0);
    let reply = runtime.operations.call_encoded(&format!("host.{}", Codec::ID), Text::from("replacement").encode()).await.unwrap();
    assert_eq!(Text::decode(&reply).unwrap().value, "replacement");
    assert_eq!(replacement.prepared.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn children_refuse_held() {
    let runtime = runtime().await;
    let owner = Arc::new(Owner::default());
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(owner.clone()).unwrap();
                dependencies
            })
            .implementation::<SameLaneDeclaration, SameLaneCall>()
            .implementation::<PrivateEchoDeclaration, PrivateEchoing>(),
            Lanes::Serial,
        ))
        .await
        .unwrap();
    let error = runtime.operation(SameLaneDeclaration::from("child")).run().await.unwrap_err();
    let Error::OperationFailed { source, .. } = error else {
        panic!("nested refusal retained");
    };
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Nested(Error::OperationLaneCycle { .. }))));
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn success_joins_unawaited() {
    let runtime = runtime().await;
    let owner = Arc::new(Owner::default());
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(owner.clone()).unwrap();
                dependencies
            })
            .implementation::<EchoDeclaration, Echoing>()
            .implementation::<PrivateEchoDeclaration, PrivateEchoing>()
            .implementation::<NestedDeclaration, Nesting>()
            .implementation::<DetachedDeclaration, Detaching>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    assert_eq!(&*runtime.operation(DetachedDeclaration::from("ignored")).run().await.unwrap(), "parent");
    assert_eq!(owner.wire_joined.load(Ordering::SeqCst), 1);
    runtime.operations.unregister(&harmos_runtime::QualifiedId::qualify("host", PrivateEchoDeclaration::ID).unwrap()).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancellation_joins_child() {
    let runtime = runtime().await;
    let owner = Arc::new(Owner::default());
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(owner.clone()).unwrap();
                dependencies
            })
            .implementation::<EchoDeclaration, Echoing>()
            .implementation::<PrivateEchoDeclaration, PrivateEchoing>()
            .implementation::<NestedDeclaration, Nesting>()
            .implementation::<DetachedDeclaration, Detaching>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    let started = owner.wire_started.notified();
    let parent = runtime.operation(NestedDeclaration::from("block")).start().await.unwrap();
    started.await;
    parent.cancel();
    assert!(matches!(parent.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert_eq!(owner.wire_joined.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

/* === Tests === */
