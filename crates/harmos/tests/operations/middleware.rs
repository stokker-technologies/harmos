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
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Lanes, app::App};
use harmos::operations::middleware::{Enter, Invocation, Middleware, Observation};
use harmos::operations::{
    CallContext, Context, Operation, OperationDeclaration, OperationPolicy, RetryPolicy,
};
use harmos::{Error, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Principal(&'static str);
#[harmos::message]
struct Stage {
    id: String,
}
struct Overlay<const CHILD: bool>;
struct Declare<const CHILD: bool>;
struct Read<const CHILD: bool>;
struct Policy {
    name: &'static str,
    events: Arc<Mutex<Vec<String>>>,
    reject: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::operations::ContextValue for Stage {
    const KEY: &'static str = "test.stage";
}

impl<const CHILD: bool> OperationDeclaration for Overlay<CHILD> {
    type Request = ();
    type Response = String;
    const ID: &'static str = if CHILD {
        "overlay.child"
    } else {
        "overlay.parent"
    };
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::Never,
    };
}

impl<const CHILD: bool> Operation for Overlay<CHILD> {
    type Declaration = Self;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> harmos::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        context: Self::Context<'a>,
        _: &mut (),
        _: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<String>> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            let actor = context.context().require::<Principal>()?.0;
            let stage = context.context().value::<Stage>()?.unwrap();
            if CHILD {
                return Ok(format!("{actor}:{}", stage.id));
            }
            assert_eq!(stage.id, "parent");
            let child = context.with_value(Stage { id: "child".into() })?;
            let stage = child.context().require::<Stage>()?;
            assert_eq!(stage.id, "child");
            let answer = child.call::<Overlay<true>>(()).await?;
            Ok(format!("{actor}:{}:{answer}", "parent"))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl<const CHILD: bool> OperationDeclaration for Declare<CHILD> {
    type Request = ();
    type Response = String;
    fn key(_: &()) -> Option<harmos::operations::CacheKey> {
        Some(harmos::operations::CacheKey::new(vec![1]).unwrap())
    }
    const ID: &'static str = if CHILD {
        "middleware.child"
    } else {
        "middleware.parent"
    };
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::Never,
    };
}

impl<const CHILD: bool> Operation for Read<CHILD> {
    type Declaration = Declare<CHILD>;
    type Resources = ();
    type Error = Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> harmos::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        context: Self::Context<'a>,
        _request: &mut (),
        _: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<String>> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            let actor = context.context().require::<Principal>()?.0;
            if CHILD {
                return Ok(actor.into());
            }
            let child = context.call::<Declare<true>>(()).await?;
            Ok(format!("{actor}:{child}"))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Middleware for Policy {
    fn enter<'a>(&'a self, call: &'a mut Invocation) -> Enter<'a> {
        Box::pin(async move {
            self.events.lock().unwrap().push(format!(
                "{} enter {}",
                self.name,
                call.operation().id
            ));
            tokio::task::yield_now().await;
            call.context().require::<Principal>()?;
            if self.reject {
                return Err(Error::InvalidInput {
                    field: "principal",
                    reason: "access denied",
                });
            }
            Ok(())
        })
    }
    fn observe(&self, call: &Invocation, observation: Observation) {
        assert!(observation.settled);
        self.events
            .lock()
            .unwrap()
            .push(format!("{} observe {}", self.name, call.operation().id));
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn runtime() -> Runtime<App> {
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_operation::<Read<false>>("app", Lanes::Open, Arc::new(()))
        .unwrap();
    runtime
        .operations
        .register_operation::<Read<true>>("app", Lanes::Open, Arc::new(()))
        .unwrap();
    runtime
}

fn context(actor: &'static str) -> CallContext {
    let mut context = CallContext::default();
    context.insert(Principal(actor)).unwrap();
    context
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("portable_overlays_preserve_local_values", || {
            use harmos::Message;
            use harmos::operations::{CallValue, ContextError, PORTABLE_BYTES, PORTABLE_VALUES};
            let mut original = context("alice");
            original
                .insert_portable(Stage {
                    id: "parent".into(),
                })
                .unwrap();
            let child = original.with_value(Stage { id: "child".into() }).unwrap();
            assert!(std::ptr::eq(
                original.require::<Principal>().unwrap(),
                child.require::<Principal>().unwrap()
            ));
            assert_eq!(original.require::<Stage>().unwrap().id, "parent");
            assert_eq!(child.require::<Stage>().unwrap().id, "child");
            assert_eq!(child.value::<Stage>().unwrap().unwrap().id, "child");
            let values = vec![CallValue {
                key: "test.stage".into(),
                payload: Stage { id: "wire".into() }.encode_to_vec(),
            }];
            let mut call = harmos::operations::WireChildCall {
                context: [7; 32],
                generation_context: Some([9; 32]),
                values: values.clone(),
                operation: "overlay.child".into(),
                payload: ().encode_to_vec(),
            };
            let encoded = call.encode().unwrap();
            let decoded = harmos::operations::WireChildCall::decode(&encoded).unwrap();
            assert_eq!(decoded.context, call.context);
            assert_eq!(decoded.generation_context, call.generation_context);
            assert_eq!(decoded.values, values);
            let mut obsolete = encoded.clone();
            obsolete[0] = 4;
            assert!(matches!(
                harmos::operations::WireChildCall::decode(&obsolete),
                Err(harmos::operations::WireCodecError::Version)
            ));
            call.values.push(values[0].clone());
            assert!(call.encode().is_err());
            call.values = vec![CallValue {
                key: "oversized".into(),
                payload: vec![0; PORTABLE_BYTES],
            }];
            assert!(matches!(
                call.encode(),
                Err(harmos::operations::WireCodecError::Oversized)
            ));
            let wire = original.with_values(decoded.values).unwrap();
            assert!(wire.get::<Stage>().is_none());
            assert_eq!(wire.value::<Stage>().unwrap().unwrap().id, "wire");
            assert!(std::ptr::eq(
                original.require::<Principal>().unwrap(),
                wire.require::<Principal>().unwrap()
            ));
            assert!(matches!(
                original.with_values(vec![values[0].clone(), values[0].clone()]),
                Err(ContextError::Duplicate { .. })
            ));
            assert!(matches!(
                original.with_values(vec![CallValue {
                    key: "oversized".into(),
                    payload: vec![0; PORTABLE_BYTES]
                }]),
                Err(ContextError::Budget)
            ));
            let replacements = (0..PORTABLE_VALUES)
                .map(|index| CallValue {
                    key: format!("value{index}"),
                    payload: Vec::new(),
                })
                .collect();
            assert!(matches!(
                original.with_values(replacements),
                Err(ContextError::Budget)
            ));
            assert_eq!(original.value::<Stage>().unwrap().unwrap().id, "parent");
            let mut trusted = CallContext::default();
            trusted.insert(Stage { id: "local".into() }).unwrap();
            assert!(matches!(
                trusted.with_value(Stage { id: "other".into() }),
                Err(ContextError::Duplicate { .. })
            ));
            assert_eq!(trusted.require::<Stage>().unwrap().id, "local");
        });
    }
    {
        scenario_runtime::check("child_overlay_preserves_principal_and_parent_stage", || {
            let mut executor = tokio::runtime::Builder::new_current_thread();
            executor.enable_all().build().unwrap().block_on(async {
                let runtime = runtime().await;
                runtime
                    .operations
                    .register_operation::<Overlay<false>>("app", Lanes::Open, Arc::new(()))
                    .unwrap();
                runtime
                    .operations
                    .register_operation::<Overlay<true>>("app", Lanes::Open, Arc::new(()))
                    .unwrap();
                let mut original = context("alice");
                original
                    .insert_portable(Stage {
                        id: "parent".into(),
                    })
                    .unwrap();
                let operations = runtime.operations.with_context(original.clone());
                assert_eq!(
                    &*operations.call::<Overlay<false>>(()).await.unwrap(),
                    "alice:parent:alice:child"
                );
                assert_eq!(original.require::<Stage>().unwrap().id, "parent");
                assert_eq!(
                    &*operations.call::<Overlay<true>>(()).await.unwrap(),
                    "alice:parent"
                );
                runtime.stop().await.unwrap();
            });
        });
    }
    {
        scenario_runtime::check("children_inherit_principals", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let events = Arc::new(Mutex::new(Vec::new()));
                    for name in ["first", "second"] {
                        runtime
                            .operations
                            .register_middleware(Arc::new(Policy {
                                name,
                                events: events.clone(),
                                reject: false,
                            }))
                            .unwrap();
                    }
                    let alice = runtime.operations.with_context(context("alice"));
                    assert_eq!(
                        &*alice.call::<Declare<false>>(()).await.unwrap(),
                        "alice:alice"
                    );
                    assert_eq!(
                        *events.lock().unwrap(),
                        [
                            "first enter middleware.parent",
                            "second enter middleware.parent",
                            "first enter middleware.child",
                            "second enter middleware.child",
                            "second observe middleware.child",
                            "first observe middleware.child",
                            "second observe middleware.parent",
                            "first observe middleware.parent",
                        ]
                    );
                    assert!(
                        runtime
                            .operations
                            .register_middleware(Arc::new(Policy {
                                name: "late",
                                events: events.clone(),
                                reject: false
                            }))
                            .is_err()
                    );
                    let bob = runtime.operations.with_context(context("bob"));
                    let (a, b) = tokio::join!(
                        alice.call::<Declare<false>>(()),
                        bob.call::<Declare<false>>(())
                    );
                    assert_eq!(&*a.unwrap(), "alice:alice");
                    assert_eq!(&*b.unwrap(), "bob:bob");
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("layers_skips_layers", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let events = Arc::new(Mutex::new(Vec::new()));
                    for (name, reject) in [("first", false), ("reject", true), ("unreached", false)]
                    {
                        runtime
                            .operations
                            .register_middleware(Arc::new(Policy {
                                name,
                                events: events.clone(),
                                reject,
                            }))
                            .unwrap();
                    }
                    assert!(
                        runtime
                            .operations
                            .with_context(context("alice"))
                            .call::<Declare<false>>(())
                            .await
                            .is_err()
                    );
                    assert_eq!(
                        *events.lock().unwrap(),
                        [
                            "first enter middleware.parent",
                            "reject enter middleware.parent",
                            "reject observe middleware.parent",
                            "first observe middleware.parent",
                        ]
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancellation_interrupts_policy", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let entered = Arc::new(tokio::sync::Semaphore::new(0));
                    let observed = Arc::new(Mutex::new(Vec::new()));
                    runtime
                        .operations
                        .register_middleware(Arc::new(Wait {
                            entered: entered.clone(),
                            observed: observed.clone(),
                        }))
                        .unwrap();
                    let invocation = runtime
                        .operations
                        .with_context(context("alice"))
                        .submit::<Declare<false>>(())
                        .unwrap();
                    tokio::time::timeout(Duration::from_secs(1), entered.acquire())
                        .await
                        .unwrap()
                        .unwrap()
                        .forget();
                    invocation.cancel();
                    assert!(matches!(
                        invocation.await,
                        Err(Error::OperationRefused(harmos::Refusal::Cancelled))
                    ));
                    {
                        let observations = observed.lock().unwrap();
                        assert_eq!(observations.len(), 1);
                        assert_eq!(
                            observations[0].outcome,
                            harmos::operations::OperationOutcome::Cancelled
                        );
                        assert!(observations[0].settled);
                    }
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("overrides_preserve_type", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    runtime
                        .operations
                        .register_operation_id::<Read<true>>(
                            "camera",
                            "camera.child",
                            Lanes::Open,
                            Arc::new(()),
                        )
                        .unwrap();
                    let operations = runtime.operations.with_context(context("alice"));
                    let result = operations
                        .submit_at::<Declare<true>>("camera.child", ())
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(&*result, "alice");
                    assert!(
                        operations
                            .submit_at::<Declare<false>>("camera.child", ())
                            .is_err()
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("retries_preserve_principal", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    runtime
                        .operations
                        .register_operation::<Retry>("app", Lanes::Open, Arc::new(()))
                        .unwrap();
                    let events = Arc::new(Mutex::new(Vec::new()));
                    runtime
                        .operations
                        .register_middleware(Arc::new(Policy {
                            name: "once",
                            events: events.clone(),
                            reject: false,
                        }))
                        .unwrap();
                    let answer = runtime
                        .operations
                        .with_context(context("alice"))
                        .call::<Retry>(())
                        .await
                        .unwrap();
                    assert_eq!(&*answer, "alice:3");
                    assert_eq!(
                        *events.lock().unwrap(),
                        [
                            "once enter middleware.retry",
                            "once observe middleware.retry"
                        ]
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
}

struct Wait {
    entered: Arc<tokio::sync::Semaphore>,
    observed: Arc<Mutex<Vec<Observation>>>,
}
impl Middleware for Wait {
    fn enter<'a>(&'a self, _: &'a mut Invocation) -> Enter<'a> {
        Box::pin(async move {
            self.entered.add_permits(1);
            std::future::pending().await
        })
    }
    fn observe(&self, _: &Invocation, observation: Observation) {
        self.observed.lock().unwrap().push(observation);
    }
}

struct Retry {
    attempts: usize,
}
impl OperationDeclaration for Retry {
    type Request = ();
    type Response = String;
    const ID: &'static str = "middleware.retry";
    const POLICY: OperationPolicy = super::Compute::POLICY;
}
impl Operation for Retry {
    type Declaration = Self;
    type Resources = ();
    type Error = std::io::Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(_: Arc<()>) -> Result<Self, Self::Error> {
        Ok(Self { attempts: 0 })
    }
    fn classify(_: &Self::Error) -> harmos::operations::Failure {
        harmos::operations::Failure::Transient
    }
    async fn run<'a>(
        &mut self,
        context: Self::Context<'a>,
        _request: &mut (),
        _: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<String>, Self::Error> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            assert_eq!(context.context().require::<Principal>().unwrap().0, "alice");
            self.attempts += 1;
            if self.attempts < 3 {
                return Err(std::io::Error::other("retry fixture"));
            }
            Ok(format!(
                "{}:{}",
                context.context().require::<Principal>().unwrap().0,
                self.attempts
            ))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
