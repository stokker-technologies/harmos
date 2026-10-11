/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{Failed, Lanes, Owner, runtime};
use harmos::operations::{
    Backoff, CacheKey, Context, Failure, Jitter, Operation, OperationDeclaration, OperationPolicy,
    RetryPolicy, Visibility,
};
use harmos::{Error, Message, Refusal};

/* === Constants === */

/* === Macros === */
thread_local! {
    static DECODE_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}

/* === Type Aliases === */

/* === Structs === */
pub(super) struct Echo;

pub(super) struct PrivateEcho;

pub(super) struct Nested;

pub(super) struct Detached;

struct SameLane;

struct Joined<'a>(&'a std::sync::atomic::AtomicUsize);

struct Codec;

struct ShortCodec;

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
    Detached(Detached),
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
    Detached(Detaching),
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
enum ImplementedCodecs {
    Codec(Coding),
    ShortCodec(ShortCoding),
}

#[harmos::catalog]
enum ImplementedCodec {
    Codec(Coding),
}

#[harmos::catalog]
enum ImplementedSerial {
    SameLane(SameLaneCall),
    PrivateEcho(PrivateEchoing),
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
        Self {
            value: value.to_owned(),
        }
    }
}

impl Message for Payload {
    fn encode_to_vec(&self) -> Vec<u8> {
        assert_ne!(self.0.value, "encode_panic", "scripted codec panic");
        self.0.encode_to_vec()
    }

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
        if value.value == "slow" {
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(Self(value))
    }
}

impl harmos::schema::Described for Payload {
    const FIELDS: &'static [harmos::schema::Field] = <Text as harmos::schema::Described>::FIELDS;
}

impl harmos::operations::WireOperation for Codec {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for ShortCodec {
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

impl harmos::operations::WireOperation for Detached {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for SameLane {
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
    };
}

impl OperationDeclaration for ShortCodec {
    type Request = Payload;
    type Response = Payload;
    const ID: &'static str = "fixture.short_codec";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_millis(5),
        attempt_timeout: Duration::from_millis(5),
        retries: RetryPolicy::Never,
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

    fn new(resources: Arc<Owner>) -> Result<Self, Failed> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { resources })
    }

    async fn run<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
        request: &mut Payload,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<Payload, Failed> {
        self.resources.runs.fetch_add(1, Ordering::SeqCst);
        Ok(Payload(Text {
            value: std::mem::take(&mut request.0.value),
        }))
    }
}

struct ShortCoding {
    resources: Arc<Owner>,
}

impl Operation for ShortCoding {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = ShortCodec;
    type Resources = Owner;
    type Error = Failed;

    fn new(resources: Arc<Owner>) -> Result<Self, Failed> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { resources })
    }

    async fn run<'ctx>(
        &mut self,
        _context: Self::Context<'ctx>,
        request: &mut Payload,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<Payload, Failed> {
        self.resources.runs.fetch_add(1, Ordering::SeqCst);
        Ok(Payload(Text {
            value: std::mem::take(&mut request.0.value),
        }))
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
            backoff: Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: Jitter::None,
            },
        },
    };

    fn key(request: &Text) -> Option<CacheKey> {
        (request.value == "retry")
            .then(|| CacheKey::new(request.value.as_bytes().to_vec()).unwrap())
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

    fn new(resources: Arc<Owner>) -> Result<Self, Failed> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        let state = State { attempts: 0 };
        Ok(Self { resources, state })
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        value: &mut Text,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<String, Failed> {
        self.resources.runs.fetch_add(1, Ordering::SeqCst);
        self.resources.seen.lock().unwrap().push(context.id());
        self.state.attempts += 1;
        if value.value == "retry" && self.state.attempts == 1 {
            value.value.push_str(" retained");
            return Err(Failed::Scripted(Failure::Transient));
        }
        if value.value == "block" {
            let _joined = Joined(&self.resources.wire_joined);
            self.resources.wire_started.notify_one();
            std::future::pending::<()>().await;
        }
        Ok(format!("{}:{}", value.value, self.state.attempts))
    }

    fn classify(error: &Failed) -> Failure {
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

    fn new(resources: Arc<Owner>) -> Result<Self, Failed> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        let state = State { attempts: 0 };
        Ok(Self { resources, state })
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        value: &mut Text,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<String, Failed> {
        self.resources.runs.fetch_add(1, Ordering::SeqCst);
        self.resources.seen.lock().unwrap().push(context.id());
        self.state.attempts += 1;
        if value.value == "retry" && self.state.attempts == 1 {
            value.value.push_str(" retained");
            return Err(Failed::Scripted(Failure::Transient));
        }
        if value.value == "block" {
            let _joined = Joined(&self.resources.wire_joined);
            self.resources.wire_started.notify_one();
            std::future::pending::<()>().await;
        }
        Ok(format!("{}:{}", value.value, self.state.attempts))
    }
}

impl Drop for Joined<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl OperationDeclaration for Nested {
    type Request = Text;
    type Response = String;
    const ID: &'static str = "fixture.nested_echo";
    const VISIBILITY: Visibility = Visibility::Shared;
    const POLICY: OperationPolicy = OperationPolicy {
        retries: RetryPolicy::Never,
        ..Echo::POLICY
    };
}

impl OperationDeclaration for Detached {
    type Request = Text;
    type Response = String;
    const ID: &'static str = "fixture.detached_echo";
    const POLICY: OperationPolicy = Nested::POLICY;
}

impl OperationDeclaration for SameLane {
    type Request = Text;
    type Response = String;
    const ID: &'static str = "fixture.same_lane_echo";
    const POLICY: OperationPolicy = Nested::POLICY;
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

    fn new(resources: Arc<Owner>) -> Result<Self, Failed> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self {
            _resources: resources,
        })
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut Text,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<String, Failed> {
        let reply = context
            .call_encoded(PrivateEcho::ID, request.encode_to_vec())
            .await?;
        Ok(String::decode(&reply).unwrap())
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

    fn new(resources: Arc<Owner>) -> Result<Self, Failed> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self {
            _resources: resources,
        })
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut Text,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<String, Failed> {
        let reply = context
            .call_encoded(PrivateEcho::ID, request.encode_to_vec())
            .await?;
        Ok(String::decode(&reply).unwrap())
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

    fn new(resources: Arc<Owner>) -> Result<Self, Failed> {
        Ok(Self { resources })
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        _request: &mut Text,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<String, Failed> {
        let started = self.resources.wire_started.notified();
        let _child =
            context.submit_encoded(PrivateEcho::ID, Text::from("block").encode_to_vec())?;
        started.await;
        Ok("parent".to_owned())
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("calls_share_attempt", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_wire_operations::<Implemented, _>(
                            "app",
                            Lanes::Open,
                            owner.clone(),
                        )
                        .unwrap();
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
                    let invocation = runtime
                        .operations
                        .submit_encoded(Echo::ID, Text::from("retry").encode_to_vec())
                        .unwrap();
                    let id = invocation.id();
                    let response = invocation.await.unwrap();
                    assert_eq!(String::decode(&response).unwrap(), "retry retained:2");
                    assert_eq!(*owner.seen.lock().unwrap(), [id, id]);
                    assert_eq!(
                        &*runtime
                            .operations
                            .call::<Echo>("retry".into())
                            .await
                            .unwrap(),
                        "retry retained:2"
                    );
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 2);
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 2);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancel_never_start", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_wire_operations::<Implemented, _>(
                            "app",
                            Lanes::Open,
                            owner.clone(),
                        )
                        .unwrap();
                    assert!(matches!(
                        runtime.operations.submit_encoded(Echo::ID, vec![0xff]),
                        Err(Error::OperationPayload { .. })
                    ));
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
                    let invocation = runtime
                        .operations
                        .submit_encoded(Echo::ID, Text::from("block").encode_to_vec())
                        .unwrap();
                    invocation.cancel();
                    assert!(matches!(
                        invocation.await,
                        Err(Error::OperationRefused(Refusal::Cancelled))
                    ));
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("binding_never_acquires", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_operations::<Implemented, _>("app", Lanes::Open, owner.clone())
                        .unwrap();
                    assert!(matches!(
                        runtime
                            .operations
                            .submit_encoded(Echo::ID, Text::from("value").encode_to_vec()),
                        Err(Error::InvalidInput {
                            field: "encoded operation",
                            ..
                        })
                    ));
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("budget_never_escape", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_wire_operations::<ImplementedCodecs, _>(
                            "app",
                            Lanes::Open,
                            owner.clone(),
                        )
                        .unwrap();
                    assert!(matches!(
                        runtime
                            .operations
                            .submit_encoded(Codec::ID, Text::from("decode_panic").encode_to_vec()),
                        Err(Error::OperationPanicked { .. })
                    ));
                    assert!(matches!(
                        runtime
                            .operations
                            .submit_encoded(ShortCodec::ID, Text::from("slow").encode_to_vec()),
                        Err(Error::OperationRefused(Refusal::TimedOut))
                    ));
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
                    assert!(matches!(
                        runtime
                            .operations
                            .call_encoded(Codec::ID, Text::from("encode_panic").encode_to_vec())
                            .await,
                        Err(Error::OperationPanicked { .. })
                    ));
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("decoding_cannot_redirect", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let old = Arc::new(Owner::default());
                    let replacement = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_wire_operations::<ImplementedCodec, _>(
                            "app",
                            Lanes::Open,
                            old.clone(),
                        )
                        .unwrap();
                    let operations = runtime.operations.clone();
                    let new_owner = replacement.clone();
                    DECODE_HOOK.with(|hook| {
                        *hook.borrow_mut() = Some(Box::new(move || {
                            std::thread::spawn(move || {
                                tokio::runtime::Builder::new_current_thread()
                                    .enable_all()
                                    .build()
                                    .unwrap()
                                    .block_on(async {
                                        operations.unregister(Codec::ID).await.unwrap();
                                        operations
                                            .register_wire_operations::<ImplementedCodec, _>(
                                                "app",
                                                Lanes::Open,
                                                new_owner,
                                            )
                                            .unwrap();
                                    });
                            })
                            .join()
                            .unwrap();
                        }));
                    });
                    assert!(matches!(
                        runtime
                            .operations
                            .submit_encoded(Codec::ID, Text::from("original").encode_to_vec()),
                        Err(Error::Closing { .. })
                    ));
                    assert_eq!(old.prepared.load(Ordering::SeqCst), 0);
                    assert_eq!(replacement.prepared.load(Ordering::SeqCst), 0);
                    let reply = runtime
                        .operations
                        .call_encoded(Codec::ID, Text::from("replacement").encode_to_vec())
                        .await
                        .unwrap();
                    assert_eq!(Text::decode(&reply).unwrap().value, "replacement");
                    assert_eq!(replacement.prepared.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("children_refuse_held", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_wire_operations::<ImplementedSerial, _>(
                            "app",
                            Lanes::Serial,
                            owner.clone(),
                        )
                        .unwrap();
                    let error = runtime
                        .operations
                        .call::<SameLane>("child".into())
                        .await
                        .unwrap_err();
                    let Error::OperationFailed { source, .. } = error else {
                        panic!("nested refusal retained");
                    };
                    assert!(matches!(
                        source.downcast_ref::<Failed>(),
                        Some(Failed::Nested(Error::OperationLaneCycle { .. }))
                    ));
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("success_joins_unawaited", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_wire_operations::<Implemented, _>(
                            "app",
                            Lanes::Open,
                            owner.clone(),
                        )
                        .unwrap();
                    assert_eq!(
                        &*runtime
                            .operations
                            .call::<Detached>("ignored".into())
                            .await
                            .unwrap(),
                        "parent"
                    );
                    assert_eq!(owner.wire_joined.load(Ordering::SeqCst), 1);
                    runtime
                        .operations
                        .unregister(PrivateEcho::ID)
                        .await
                        .unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancellation_joins_child", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_wire_operations::<Implemented, _>(
                            "app",
                            Lanes::Open,
                            owner.clone(),
                        )
                        .unwrap();
                    let started = owner.wire_started.notified();
                    let parent = runtime.operations.submit::<Nested>("block".into()).unwrap();
                    started.await;
                    parent.cancel();
                    assert!(matches!(
                        parent.await,
                        Err(Error::OperationRefused(Refusal::Cancelled))
                    ));
                    assert_eq!(owner.wire_joined.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
}

/* === Tests === */
