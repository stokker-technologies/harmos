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
use harmos::operations::{
    Backoff, CacheKey, Context, FailureKind, Jitter, Operation, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy,
    Visibility,
};
use harmos::{Error, Message};

/* === Constants === */
const ECHO_POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::OnTransient { max_retries: 1, backoff: Backoff::Fixed { delay: Duration::from_millis(1), jitter: Jitter::None } },
    restart: RestartPolicy::Never,
};

const NESTED_POLICY: OperationPolicy = OperationPolicy { retries: RetryPolicy::Never, ..ECHO_POLICY };

const CODEC_POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

const SHORT_CODEC_POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_millis(5),
    attempt_timeout: Duration::from_millis(5),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */
thread_local! {
    static DECODE_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}

// Each Echo Request Carries One Text Value.
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
            fn encode_to_vec(&self) -> Vec<u8> {
                assert_ne!(self.0.value, "encode_panic", "scripted codec panic");
                Message::encode_to_vec(&self.0)
            }

            fn encoded_len(&self) -> usize {
                Message::encoded_len(&self.0)
            }

            fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos::MessageError> {
                <Text as Message>::decoding_memory(bytes)
            }

            fn decode(bytes: &[u8]) -> Result<Self, harmos::MessageError> {
                if let Some(run) = DECODE_HOOK.with(|hook| hook.borrow_mut().take()) {
                    run();
                }
                let value = <Text as Message>::decode(bytes)?;
                assert_ne!(value.value, "decode_panic", "scripted codec panic");
                if value.value == "slow" {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Ok(Self(value))
            }
        }

        #[cfg(feature = "grpc")]
        impl harmos::Protobuf for $ty {
            fn encode_to_vec(&self) -> Vec<u8> {
                harmos::Protobuf::encode_to_vec(&self.0)
            }

            fn encoded_len(&self) -> usize {
                harmos::Protobuf::encoded_len(&self.0)
            }

            fn decode(bytes: &[u8]) -> Result<Self, harmos::MessageError> {
                <Text as harmos::Protobuf>::decode(bytes).map(Self)
            }
        }

        impl harmos::MemorySize for $ty {
            fn memory_size(&self) -> usize {
                self.0.memory_size()
            }
        }

        impl harmos::schema::Described for $ty {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($ty));
            const FIELDS: &'static [harmos::schema::Field] = <Text as harmos::schema::Described>::FIELDS;
            const MARKER: &'static [u8] = &harmos::schema::declaration::encode::<
                { harmos::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
            >(Self::NAME, Self::FIELDS);
        }
    )*};
}

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[harmos::operation(
    id = "fixture.echo",
    version = 1,
    response = String,
    visibility = Visibility::Shared,
    policy = ECHO_POLICY,
    key = |request: &Echo| (request.value == "retry")
        .then(|| CacheKey::new(request.value.as_bytes().to_vec()).unwrap()),
)]
#[derive(Debug)]
pub(super) struct Echo {
    pub value: String,
}

#[harmos::message]
#[harmos::operation(
    id = "fixture.private_echo",
    version = 1,
    response = String,
    policy = ECHO_POLICY
)]
#[derive(Debug)]
pub(super) struct PrivateEcho {
    pub value: String,
}

#[harmos::message]
#[harmos::operation(
    id = "fixture.nested_echo",
    version = 1,
    response = String,
    visibility = Visibility::Shared,
    policy = NESTED_POLICY
)]
#[derive(Debug)]
pub(super) struct Nested {
    pub value: String,
}

struct Codec;

pub(super) struct Payload(Text);

#[harmos::message]
#[derive(Debug)]
pub(super) struct Text {
    pub value: String,
}

pub(super) struct State {
    attempts: usize,
}

/* === Enums === */
#[harmos::catalog]
pub(super) enum Leaves {
    Echo(Echo),
    PrivateEcho(PrivateEcho),
    Nested(Nested),
}

#[harmos::catalog]
pub(super) enum Group {
    #[nested]
    Leaves(Leaves),
}

#[harmos::catalog]
pub(super) enum Operations {
    #[nested]
    Empty(()),
    #[nested]
    Group(Group),
}

#[harmos::catalog]
pub(super) enum ImplementedLeaves {
    Echo(Echoing),
    PrivateEcho(PrivateEchoing),
    Nested(Nesting),
}

#[harmos::catalog]
pub(super) enum ImplementedGroup {
    #[nested]
    Leaves(ImplementedLeaves),
}

#[harmos::catalog]
pub(super) enum Implemented {
    #[nested]
    Empty(()),
    #[nested]
    Group(ImplementedGroup),
}

#[harmos::catalog]
enum ImplementedCodec {
    Codec(Coding),
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

text!(Echo, PrivateEcho, Nested, Detached, SameLane);

    fn encoded_len(&self) -> usize {
        self.0.encoded_len()
    }

    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos::MessageError> {
        Text::decoding_memory(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, harmos::MessageError> {
        if let Some(run) = DECODE_HOOK.with(|hook| hook.borrow_mut().take()) {
            run();
        }
        let value = Text::decode(bytes)?;
        assert_ne!(value.value, "decode_panic", "scripted codec panic");
        Ok(Self(value))
    }
}

impl harmos::schema::Described for Payload {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Payload));
    const FIELDS: &'static [harmos::schema::Field] = <Text as harmos::schema::Described>::FIELDS;
    const MARKER: &'static [u8] = &harmos::schema::declaration::encode::<
        { harmos::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl harmos::operations::WireOperation for Codec {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for Echo {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for PrivateEcho {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for Nested {
    const VERSION: u32 = 1;
}

impl OperationDeclaration for Codec {
    type Request = Payload;
    type Response = Payload;
    const ID: &'static str = "fixture.codec";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
}

struct Coding {
    resources: Arc<Owner>,
}

impl Operation for Coding {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = Codec;
    type Resources = Owner;
    type Error = Failed;

    async fn initialize<'ctx>(resources: Arc<Owner>, _: Self::Context<'ctx>) -> Result<Self, Failed> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { resources })
    }

    async fn run<'ctx>(
        &mut self, _context: Self::Context<'ctx>, request: &mut Codec, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Payload>, Failed> {
        let result: ::core::result::Result<Payload, Self::Error> = async move {
            self.resources.runs.fetch_add(1, Ordering::SeqCst);
            Ok(Payload(Text { value: std::mem::take(&mut request.0.value) }))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl OperationDeclaration for Echo {
    type Request = Text;
    type Response = String;
    const ID: &'static str = "fixture.echo";
    const VISIBILITY: Visibility = Visibility::Shared;
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: Backoff::Fixed { delay: Duration::from_millis(1), jitter: Jitter::None },
        },
        restart: RestartPolicy::Never,
    };

    fn key(request: &Text) -> Option<CacheKey> {
        (request.value == "retry").then(|| CacheKey::new(request.value.as_bytes().to_vec()).unwrap())
    }
}

impl OperationDeclaration for PrivateEcho {
    type Request = Text;
    type Response = String;
    const ID: &'static str = "fixture.private_echo";
    const POLICY: OperationPolicy = Echo::POLICY;
}

pub(super) struct Echoing {
    resources: Arc<Owner>,
    state: State,
}

impl Operation for Echoing {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = Echo;
    type Resources = Owner;
    type Error = Failed;

    async fn initialize<'ctx>(resources: Arc<Owner>, _: Self::Context<'ctx>) -> Result<Self, Failed> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        let state = State { attempts: 0 };
        Ok(Self { resources, state })
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, value: &mut Echo, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<String>, Failed> {
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

impl Operation for PrivateEchoing {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = PrivateEcho;
    type Resources = Owner;
    type Error = Failed;

    async fn initialize<'ctx>(resources: Arc<Owner>, _: Self::Context<'ctx>) -> Result<Self, Failed> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        let state = State { attempts: 0 };
        Ok(Self { resources, state })
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, value: &mut PrivateEcho, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<String>, Failed> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            self.resources.runs.fetch_add(1, Ordering::SeqCst);
            self.resources.seen.lock().unwrap().push(context.id());
            self.state.attempts += 1;
            Ok(format!("{}:{}", value.value, self.state.attempts))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl OperationDeclaration for Nested {
    type Request = Text;
    type Response = String;
    const ID: &'static str = "fixture.nested_echo";
    const VISIBILITY: Visibility = Visibility::Shared;
    const POLICY: OperationPolicy = OperationPolicy { retries: RetryPolicy::Never, ..Echo::POLICY };
}

pub(super) struct Nesting {
    _resources: Arc<Owner>,
}

impl Operation for Nesting {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = Nested;
    type Resources = Owner;
    type Error = Failed;

    async fn initialize<'ctx>(resources: Arc<Owner>, _: Self::Context<'ctx>) -> Result<Self, Failed> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { _resources: resources })
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, request: &mut Nested, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<String>, Failed> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            let reply = context.call_encoded(PrivateEcho::ID, Message::encode_to_vec(request)).await?;
            Ok(String::decode(&reply).unwrap())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

struct SameLaneCall {
    _resources: Arc<Owner>,
}

impl Operation for SameLaneCall {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = SameLane;
    type Resources = Owner;
    type Error = Failed;

    async fn initialize<'ctx>(resources: Arc<Owner>, _: Self::Context<'ctx>) -> Result<Self, Failed> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { _resources: resources })
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, request: &mut SameLane, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<String>, Failed> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            let reply = context.call_encoded(PrivateEcho::ID, Message::encode_to_vec(request)).await?;
            Ok(String::decode(&reply).unwrap())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

pub(super) struct Detaching {
    resources: Arc<Owner>,
}

impl Operation for Detaching {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = Detached;
    type Resources = Owner;
    type Error = Failed;

    async fn initialize<'ctx>(resources: Arc<Owner>, _: Self::Context<'ctx>) -> Result<Self, Failed> {
        Ok(Self { resources })
    }

    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, _request: &mut Detached, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<String>, Failed> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            let started = self.resources.wire_started.notified();
            let _child = context.submit_encoded(PrivateEcho::ID, Text::from("block").encode_to_vec())?;
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
    runtime.operations.register_wire_operations::<Implemented, _>("app", Lanes::Open, owner.clone()).unwrap();
    assert!(matches!(runtime.operations.submit_encoded(Echo::ID, vec![0xff]), Err(Error::OperationPayload { .. })));
    // The refused payload constructs nothing.
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn binding_never_acquires() {
    let runtime = runtime().await;
    let owner = Arc::new(Owner::default());
    runtime.operations.register_operations::<Implemented, _>("app", Lanes::Open, owner.clone()).unwrap();
    assert!(matches!(
        runtime.operations.submit_encoded(Echo::ID, Text::from("value").encode_to_vec()),
        Err(Error::InvalidInput { field: "encoded operation", .. })
    ));
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn decode_panic_refused() {
    let runtime = runtime().await;
    let owner = Arc::new(Owner::default());
    runtime.operations.register_wire_operations::<ImplementedCodec, _>("app", Lanes::Open, owner.clone()).unwrap();
    assert!(matches!(
        runtime.operations.submit_encoded(Codec::ID, Text::from("decode_panic").encode_to_vec()),
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
    runtime.operations.register_wire_operations::<ImplementedCodec, _>("app", Lanes::Open, owner.clone()).unwrap();
    assert!(matches!(
        runtime.operations.call_encoded(Codec::ID, Text::from("encode_panic").encode_to_vec()).await,
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
    runtime.operations.register_wire_operations::<ImplementedCodec, _>("app", Lanes::Open, old.clone()).unwrap();
    let operations = runtime.operations.clone();
    let new_owner = replacement.clone();
    DECODE_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
                    operations.unregister(Codec::ID).await.unwrap();
                    operations.register_wire_operations::<ImplementedCodec, _>("app", Lanes::Open, new_owner).unwrap();
                });
            })
            .join()
            .unwrap();
        }));
    });
    assert!(matches!(runtime.operations.submit_encoded(Codec::ID, Text::from("original").encode_to_vec()), Err(Error::Closing { .. })));
    assert_eq!(old.prepared.load(Ordering::SeqCst), 0);
    assert_eq!(replacement.prepared.load(Ordering::SeqCst), 0);
    let reply = runtime.operations.call_encoded(Codec::ID, Text::from("replacement").encode_to_vec()).await.unwrap();
    assert_eq!(Text::decode(&reply).unwrap().value, "replacement");
    assert_eq!(replacement.prepared.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn children_refuse_held() {
    let runtime = runtime().await;
    let owner = Arc::new(Owner::default());
    runtime.operations.register_wire_operations::<ImplementedSerial, _>("app", Lanes::Serial, owner.clone()).unwrap();
    let error = runtime.operations.call(SameLane::from("child")).await.unwrap_err();
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
    runtime.operations.register_wire_operations::<Implemented, _>("app", Lanes::Open, owner.clone()).unwrap();
    assert_eq!(&*runtime.operations.call(Detached::from("ignored")).await.unwrap(), "parent");
    assert_eq!(owner.wire_joined.load(Ordering::SeqCst), 1);
    runtime.operations.unregister(PrivateEcho::ID).await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancellation_joins_child() {
    let runtime = runtime().await;
    let owner = Arc::new(Owner::default());
    runtime.operations.register_wire_operations::<Implemented, _>("app", Lanes::Open, owner.clone()).unwrap();
    let started = owner.wire_started.notified();
    let parent = runtime.operations.submit(Nested::from("block")).unwrap();
    started.await;
    parent.cancel();
    assert!(matches!(parent.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert_eq!(owner.wire_joined.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

/* === Tests === */
