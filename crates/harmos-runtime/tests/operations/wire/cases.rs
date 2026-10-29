/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;

use super::{Failed, Lanes, Owner};
use harmos_runtime::operations::{Backoff, CacheKey, FailureKind, Jitter, OperationDeclaration, OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Message, Refusal};

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

#[harmos_runtime::operation(resources = [Owner, harmos_runtime::operations::HostContext], id = "fixture.echo",
    response = String,
    visibility = Public,
    policy = ECHO_POLICY,
    key = |request: &EchoDeclaration| (request.value == "retry")
        .then(|| CacheKey::new(request.value.as_bytes().to_vec()).unwrap()),
)]
pub(super) struct EchoDeclaration {
    pub value: String,
}

#[harmos_runtime::operation(resources = [Owner, harmos_runtime::operations::HostContext], id = "fixture.private_echo",
    response = String,
    policy = ECHO_POLICY
)]
#[derive(Debug)]
pub(super) struct PrivateEchoDeclaration {
    pub value: String,
}

#[harmos_runtime::operation(resources = [Owner, harmos_runtime::operations::HostContext, Destination], id = "fixture.nested_echo",
    response = String,
    visibility = Public,
    policy = NESTED_POLICY
)]
#[derive(Debug)]
pub(super) struct NestedDeclaration {
    pub value: String,
}

#[harmos_runtime::operation(resources = [Owner, harmos_runtime::operations::HostContext], id = "fixture.detached_echo",
    response = String,
    policy = NESTED_POLICY
)]
#[derive(Debug)]
pub(super) struct DetachedDeclaration {
    pub value: String,
}

#[harmos_runtime::operation(resources = [Owner, harmos_runtime::operations::HostContext], id = "fixture.same_lane_echo",
    response = String,
    policy = NESTED_POLICY
)]
#[derive(Debug)]
struct SameLaneDeclaration {
    pub value: String,
}

struct Joined<'a>(&'a std::sync::atomic::AtomicUsize);

#[harmos_runtime::operation(resources = [Owner, harmos_runtime::operations::HostContext], codec = manual, id = "fixture.codec",  response = Payload, policy = CODEC_POLICY)]
#[derive(Clone)]
struct Codec(Text);

pub(super) struct Payload(Text);

#[harmos_runtime::message]
#[derive(Debug)]
pub(super) struct Text {
    pub value: String,
}

pub(super) struct Destination(pub &'static str);

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

impl harmos_runtime::Operation for Codec {
    type Error = Failed;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<Payload>, Failed> {
        let resources = ctx.resource::<Owner>();
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        if context.attempt() == 0 {
            resources.prepared.fetch_add(1, Ordering::SeqCst);
        }
        let result: ::core::result::Result<Payload, Self::Error> = async move {
            resources.runs.fetch_add(1, Ordering::SeqCst);
            Ok(Payload(Text { value: self.0.value }))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for EchoDeclaration {
    type Error = Failed;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<String>, Failed> {
        let resources = ctx.resource::<Owner>();
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        if context.attempt() == 0 {
            resources.prepared.fetch_add(1, Ordering::SeqCst);
        }
        let result: ::core::result::Result<String, Self::Error> = async move {
            resources.runs.fetch_add(1, Ordering::SeqCst);
            resources.seen.lock().unwrap().push(context.id());
            Ok(format!("{}:{}", self.value, context.attempt() + 1))
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    fn classify(error: &Failed) -> FailureKind {
        Owner::classify(error)
    }
}

impl harmos_runtime::Operation for PrivateEchoDeclaration {
    type Error = Failed;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<String>, Failed> {
        let resources = ctx.resource::<Owner>();
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        if context.attempt() == 0 {
            resources.prepared.fetch_add(1, Ordering::SeqCst);
        }
        let result: ::core::result::Result<String, Self::Error> = async move {
            resources.runs.fetch_add(1, Ordering::SeqCst);
            resources.seen.lock().unwrap().push(context.id());
            if self.value == "block" {
                let _joined = Joined(&resources.wire_joined);
                resources.wire_started.notify_one();
                std::future::pending::<()>().await;
            }
            Ok(format!("{}:{}", self.value, context.attempt() + 1))
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

impl harmos_runtime::Operation for NestedDeclaration {
    type Error = Failed;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<String>, Failed> {
        let resources = ctx.resource::<Owner>();
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        if context.attempt() == 0 {
            resources.prepared.fetch_add(1, Ordering::SeqCst);
        }
        let destination = ctx.resource::<Destination>();
        let result: ::core::result::Result<String, Self::Error> = async move {
            let reply = context.call_encoded(&format!("{}.{}", destination.0, PrivateEchoDeclaration::ID), Message::encode(&self)).await?;
            Ok(String::decode(&reply).unwrap())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for SameLaneDeclaration {
    type Error = Failed;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<String>, Failed> {
        let resources = ctx.resource::<Owner>();
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        if context.attempt() == 0 {
            resources.prepared.fetch_add(1, Ordering::SeqCst);
        }
        let result: ::core::result::Result<String, Self::Error> = async move {
            let reply = context.call_encoded(&format!("host.{}", PrivateEchoDeclaration::ID), Message::encode(&self)).await?;
            Ok(String::decode(&reply).unwrap())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for DetachedDeclaration {
    type Error = Failed;

    async fn run(self, context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<String>, Failed> {
        let resources = context.resource::<Owner>();
        let host = context.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let result: ::core::result::Result<String, Self::Error> = async move {
            let started = resources.wire_started.notified();
            let _child = context.submit_encoded(&format!("host.{}", PrivateEchoDeclaration::ID), Text::from("block").encode())?;
            started.await;
            Ok("parent".to_owned())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

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
    runtime.operations.register::<crate::operations::Single<EchoDeclaration>>(Lanes::Open).await.unwrap();
    assert!(matches!(
        runtime.operations.submit_encoded(&format!("host.{}", EchoDeclaration::ID), vec![0xff]),
        Err(Error::OperationPayload { .. })
    ));
    // The refused payload constructs nothing.
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn decode_panic_refused() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_owner(owner.clone()).await;
    runtime.operations.register::<crate::operations::Single<Codec>>(Lanes::Open).await.unwrap();
    assert!(matches!(
        runtime.operations.submit_encoded(&format!("host.{}", Codec::ID), Text::from("decode_panic").encode()),
        Err(Error::OperationPanicked { .. })
    ));
    // The panic stays inside submission and constructs nothing.
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn encode_panic_settles() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_owner(owner.clone()).await;
    runtime.operations.register::<crate::operations::Single<Codec>>(Lanes::Open).await.unwrap();
    assert!(matches!(
        runtime.operations.call_encoded(&format!("host.{}", Codec::ID), Text::from("encode_panic").encode()).await,
        Err(Error::OperationPanicked { .. })
    ));
    // The response panics after one completed run.
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn decode_races_replacement() {
    let old = Arc::new(Owner::default());
    let replacement = old.clone();
    let runtime = runtime_with_owner(old.clone()).await;
    runtime.operations.register::<crate::operations::Single<Codec>>(Lanes::Open).await.unwrap();
    let operations = runtime.operations.clone();
    DECODE_HOOK.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
                    operations.unregister(&harmos_runtime::QualifiedId::qualify("host", Codec::ID).unwrap()).await.unwrap();
                    operations.register::<crate::operations::Single<Codec>>(Lanes::Open).await.unwrap();
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
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn children_refuse_held() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_owner(owner.clone()).await;
    runtime.operations.register::<crate::operations::Single<PrivateEchoDeclaration>>(Lanes::Serial).await.unwrap();
    runtime.operations.register::<crate::operations::Single<SameLaneDeclaration>>(Lanes::Serial).await.unwrap();
    let error = runtime.operation(SameLaneDeclaration::from("child")).run().await.unwrap_err();
    let Error::OperationFailed { source, .. } = error else {
        panic!("nested refusal retained");
    };
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Nested(Error::OperationLaneCycle { .. }))));
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn success_joins_unawaited() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_owner(owner.clone()).await;
    runtime.operations.register::<crate::operations::Single<PrivateEchoDeclaration>>(Lanes::Open).await.unwrap();
    runtime.operations.register::<crate::operations::Single<DetachedDeclaration>>(Lanes::Open).await.unwrap();
    assert_eq!(&*runtime.operation(DetachedDeclaration::from("ignored")).run().await.unwrap(), "parent");
    assert_eq!(owner.wire_joined.load(Ordering::SeqCst), 1);
    runtime.operations.unregister(&harmos_runtime::QualifiedId::qualify("host", PrivateEchoDeclaration::ID).unwrap()).await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancellation_joins_child() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with_owner(owner.clone()).await;
    runtime.operations.register::<crate::operations::Single<PrivateEchoDeclaration>>(Lanes::Open).await.unwrap();
    runtime.operations.register::<crate::operations::Single<NestedDeclaration>>(Lanes::Open).await.unwrap();
    let started = owner.wire_started.notified();
    let parent = runtime.operation(NestedDeclaration::from("block")).start().await.unwrap();
    started.await;
    parent.cancel();
    assert!(matches!(parent.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert_eq!(owner.wire_joined.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}
