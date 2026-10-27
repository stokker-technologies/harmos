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
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::operations::middleware::{Enter, Invocation, Middleware, Observation};
use harmos_runtime::operations::{CallContext, Operation, OperationDeclaration, OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Runtime};

/* === Constants === */

const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Principal(&'static str);
#[harmos_runtime::message]
struct Stage {
    id: String,
}
struct Overlay<const CHILD: bool>;
struct Read<const CHILD: bool>;
// Each Side Of A Parent/Child Pair Selects Its Own Declarations.
struct Side<const CHILD: bool>;
#[harmos_runtime::operation(request = OverlayParent, infer_request, id = "overlay_parent",  response = String, policy = POLICY)]
struct OverlayParentDeclaration;

#[harmos_runtime::message]

struct OverlayParent {}
#[harmos_runtime::operation(request = OverlayChild, infer_request, id = "overlay_child",  response = String, policy = POLICY)]
struct OverlayChildDeclaration;

#[harmos_runtime::message]

struct OverlayChild {}
#[harmos_runtime::operation(request = DeclareParent, infer_request,
    id = "middleware_parent",

    response = String,
    policy = POLICY,
    key = keyed
)]
struct DeclareParentDeclaration;

#[harmos_runtime::message]

struct DeclareParent {}
#[harmos_runtime::operation(request = DeclareChild, infer_request,
    id = "middleware_child",

    response = String,
    policy = POLICY,
    key = keyed
)]
struct DeclareChildDeclaration;

#[harmos_runtime::message]

struct DeclareChild {}
struct Policy {
    name: &'static str,
    events: Arc<Mutex<Vec<String>>>,
    reject: bool,
}

/* === Enums === */

/* === Traits === */

trait Sided {
    type Overlay: OperationDeclaration<Response = String>;
    type Declare: OperationDeclaration<Response = String>;
}

/* === Implementations === */

impl harmos_runtime::operations::ContextValue for Stage {
    const KEY: &'static str = "test.stage";
}

impl Sided for Side<false> {
    type Overlay = OverlayParentDeclaration;
    type Declare = DeclareParentDeclaration;
}

impl Sided for Side<true> {
    type Overlay = OverlayChildDeclaration;
    type Declare = DeclareChildDeclaration;
}

impl<const CHILD: bool> Operation for Overlay<CHILD>
where
    Side<CHILD>: Sided,
{
    type Request = <<Side<CHILD> as Sided>::Overlay as OperationDeclaration>::Request;
    type Response = String;
    type Error = Error;
    type Endpoints = ();
    async fn initialize(_: harmos_runtime::OperationContext) -> harmos_runtime::Result<Self> {
        Ok(Self)
    }
    async fn run(
        &mut self, context: harmos_runtime::OperationContext, _: &mut Self::Request, _: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<String>> {
        let host = context.dependency::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
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
            let answer = child.call::<OverlayChildDeclaration>(OverlayChild {}).await?;
            Ok(format!("{actor}:{}:{answer}", "parent"))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl<const CHILD: bool> Operation for Read<CHILD>
where
    Side<CHILD>: Sided,
{
    type Request = <<Side<CHILD> as Sided>::Declare as OperationDeclaration>::Request;
    type Response = String;
    type Error = Error;
    type Endpoints = ();
    async fn initialize(_: harmos_runtime::OperationContext) -> harmos_runtime::Result<Self> {
        Ok(Self)
    }
    async fn run(
        &mut self, context: harmos_runtime::OperationContext, _request: &mut Self::Request, _: &mut Self::Endpoints,
    ) -> harmos_runtime::Result<harmos_runtime::operations::Response<String>> {
        let host = context.dependency::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<String, Self::Error> = async move {
            let actor = context.context().require::<Principal>()?.0;
            if CHILD {
                return Ok(actor.into());
            }
            let child = context.call::<DeclareChildDeclaration>(DeclareChild {}).await?;
            Ok(format!("{actor}:{child}"))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Middleware for Policy {
    fn enter<'a>(&'a self, call: &'a mut Invocation) -> Enter<'a> {
        Box::pin(async move {
            self.events.lock().unwrap().push(format!("{} enter {}", self.name, call.operation().id));
            tokio::task::yield_now().await;
            call.context().require::<Principal>()?;
            if self.reject {
                return Err(Error::InvalidInput { field: "principal", reason: "access denied" });
            }
            Ok(())
        })
    }
    fn observe(&self, call: &Invocation, observation: Observation) {
        assert!(observation.settled);
        self.events.lock().unwrap().push(format!("{} observe {}", self.name, call.operation().id));
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn keyed<O>(_: &O) -> Option<harmos_runtime::operations::CacheKey> {
    Some(harmos_runtime::operations::CacheKey::new(vec![1]).unwrap())
}

async fn runtime() -> Runtime<App> {
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new(harmos_runtime::Dependencies::new())
                .implementation::<DeclareParentDeclaration, Read<false>>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new(harmos_runtime::Dependencies::new())
                .implementation::<DeclareChildDeclaration, Read<true>>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    runtime
}

fn context(actor: &'static str) -> CallContext {
    let mut context = CallContext::default();
    context.insert(Principal(actor)).unwrap();
    context
}

/* === Tests === */

#[test]
fn portable_preserves_local() {
    use harmos_runtime::Message;
    use harmos_runtime::operations::{CallValue, ContextError, PORTABLE_BYTES, PORTABLE_VALUES};
    let mut original = context("alice");
    original.insert_portable(Stage { id: "parent".into() }).unwrap();
    let child = original.with_value(Stage { id: "child".into() }).unwrap();
    assert!(std::ptr::eq(original.require::<Principal>().unwrap(), child.require::<Principal>().unwrap()));
    assert_eq!(original.require::<Stage>().unwrap().id, "parent");
    assert_eq!(child.require::<Stage>().unwrap().id, "child");
    assert_eq!(child.value::<Stage>().unwrap().unwrap().id, "child");
    let values = vec![CallValue { key: "test.stage".into(), payload: Stage { id: "wire".into() }.encode() }];
    let mut call = harmos_runtime::operations::WireChildCall {
        context: [7; 32],
        generation_context: Some([9; 32]),
        values: values.clone(),
        operation: "overlay.child".into(),
        fingerprint: [1; 32],
        generation: u64::MAX,
        payload: ().encode(),
        submit: true,
    };
    let encoded = call.encode().unwrap();
    let decoded = harmos_runtime::operations::WireChildCall::decode(&encoded).unwrap();
    assert_eq!(decoded.context, call.context);
    assert_eq!(decoded.generation_context, call.generation_context);
    assert_eq!(decoded.values, values);
    assert!(decoded.submit);
    call.values.push(values[0].clone());
    assert!(call.encode().is_err());
    call.values = vec![CallValue { key: "oversized".into(), payload: vec![0; PORTABLE_BYTES] }];
    assert!(matches!(call.encode(), Err(harmos_runtime::operations::WireCodecError::Oversized)));
    let wire = original.with_values(decoded.values).unwrap();
    assert!(wire.get::<Stage>().is_none());
    assert_eq!(wire.value::<Stage>().unwrap().unwrap().id, "wire");
    assert!(std::ptr::eq(original.require::<Principal>().unwrap(), wire.require::<Principal>().unwrap()));
    assert!(matches!(original.with_values(vec![values[0].clone(), values[0].clone()]), Err(ContextError::Duplicate { .. })));
    assert!(matches!(
        original.with_values(vec![CallValue { key: "oversized".into(), payload: vec![0; PORTABLE_BYTES] }]),
        Err(ContextError::Budget)
    ));
    let replacements = (0..PORTABLE_VALUES).map(|index| CallValue { key: format!("value{index}"), payload: Vec::new() }).collect();
    assert!(matches!(original.with_values(replacements), Err(ContextError::Budget)));
    assert_eq!(original.value::<Stage>().unwrap().unwrap().id, "parent");
    let mut trusted = CallContext::default();
    trusted.insert(Stage { id: "local".into() }).unwrap();
    assert!(matches!(trusted.with_value(Stage { id: "other".into() }), Err(ContextError::Duplicate { .. })));
    assert_eq!(trusted.require::<Stage>().unwrap().id, "local");
}

#[tokio::test]
async fn child_preserves_authority() {
    let runtime = runtime().await;
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new(harmos_runtime::Dependencies::new())
                .implementation::<OverlayParentDeclaration, Overlay<false>>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new(harmos_runtime::Dependencies::new())
                .implementation::<OverlayChildDeclaration, Overlay<true>>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    let mut original = context("alice");
    original.insert_portable(Stage { id: "parent".into() }).unwrap();
    let operations = runtime.operations.with_context(original.clone());
    assert_eq!(&*operations.call::<OverlayParentDeclaration>(OverlayParent {}).await.unwrap(), "alice:parent:alice:child");
    assert_eq!(original.require::<Stage>().unwrap().id, "parent");
    assert_eq!(&*operations.call::<OverlayChildDeclaration>(OverlayChild {}).await.unwrap(), "alice:parent");
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn children_inherit_principals() {
    let runtime = runtime().await;
    let events = Arc::new(Mutex::new(Vec::new()));
    for name in ["first", "second"] {
        runtime.operations.register_middleware(Arc::new(Policy { name, events: events.clone(), reject: false })).unwrap();
    }
    let alice = runtime.operations.with_context(context("alice"));
    assert_eq!(&*alice.call::<DeclareParentDeclaration>(DeclareParent {}).await.unwrap(), "alice:alice");
    assert_eq!(
        *events.lock().unwrap(),
        [
            "first enter middleware_parent",
            "second enter middleware_parent",
            "first enter middleware_child",
            "second enter middleware_child",
            "second observe middleware_child",
            "first observe middleware_child",
            "second observe middleware_parent",
            "first observe middleware_parent",
        ]
    );
    assert!(runtime.operations.register_middleware(Arc::new(Policy { name: "late", events: events.clone(), reject: false })).is_err());
    let bob = runtime.operations.with_context(context("bob"));
    let (a, b) =
        tokio::join!(alice.call::<DeclareParentDeclaration>(DeclareParent {}), bob.call::<DeclareParentDeclaration>(DeclareParent {}));
    assert_eq!(&*a.unwrap(), "alice:alice");
    assert_eq!(&*b.unwrap(), "bob:bob");
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn layers_skips_layers() {
    let runtime = runtime().await;
    let events = Arc::new(Mutex::new(Vec::new()));
    for (name, reject) in [("first", false), ("reject", true), ("unreached", false)] {
        runtime.operations.register_middleware(Arc::new(Policy { name, events: events.clone(), reject })).unwrap();
    }
    assert!(runtime.operations.with_context(context("alice")).call::<DeclareParentDeclaration>(DeclareParent {}).await.is_err());
    assert_eq!(
        *events.lock().unwrap(),
        [
            "first enter middleware_parent",
            "reject enter middleware_parent",
            "reject observe middleware_parent",
            "first observe middleware_parent",
        ]
    );
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancellation_interrupts_policy() {
    let runtime = runtime().await;
    let entered = Arc::new(tokio::sync::Semaphore::new(0));
    let observed = Arc::new(Mutex::new(Vec::new()));
    runtime.operations.register_middleware(Arc::new(Wait { entered: entered.clone(), observed: observed.clone() })).unwrap();
    let invocation = runtime.operations.with_context(context("alice")).submit::<DeclareParentDeclaration>(DeclareParent {}).unwrap();
    (entered.acquire()).await.unwrap().forget();
    invocation.cancel();
    assert!(matches!(invocation.await, Err(Error::OperationRefused(harmos_runtime::Refusal::Cancelled))));
    {
        let observations = observed.lock().unwrap();
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].outcome, harmos_runtime::operations::OperationOutcome::Cancelled);
        assert!(observations[0].settled);
    }
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn overrides_preserve_type() {
    let runtime = runtime().await;
    runtime.operations.unregister(&harmos_runtime::QualifiedId::qualify("host", DeclareChildDeclaration::ID).unwrap()).await.unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new(harmos_runtime::Dependencies::new())
                .implementation::<DeclareChildDeclaration, Read<true>>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    let operations = runtime.operations.with_context(context("alice"));
    let result = operations.submit_at::<DeclareChildDeclaration>(DeclareChildDeclaration::ID, DeclareChild {}).unwrap().await.unwrap();
    assert_eq!(&*result, "alice");
    assert!(operations.submit_at::<DeclareParentDeclaration>(DeclareChildDeclaration::ID, DeclareParent {}).is_err());
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn retries_preserve_principal() {
    let runtime = runtime().await;
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(Arc::new(())).unwrap();
                dependencies
            })
            .implementation::<RetryDeclaration, Retrying>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    runtime.operations.register_middleware(Arc::new(Policy { name: "once", events: events.clone(), reject: false })).unwrap();
    let answer = runtime.operations.with_context(context("alice")).call::<RetryDeclaration>(Retry {}).await.unwrap();
    assert_eq!(&*answer, "alice:3");
    assert_eq!(*events.lock().unwrap(), ["once enter middleware_retry", "once observe middleware_retry"]);
    runtime.stop().await.unwrap();
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

#[harmos_runtime::operation(request = Retry, infer_request,
    id = "middleware_retry",

    response = String,
    policy = super::Compute::POLICY
)]
struct RetryDeclaration;

#[harmos_runtime::message]

struct Retry {}
struct Retrying {
    attempts: usize,
}
impl Operation for Retrying {
    type Request = Retry;
    type Response = <RetryDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = std::io::Error;
    type Endpoints = ();
    async fn initialize(_: harmos_runtime::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self { attempts: 0 })
    }
    fn classify(_: &Self::Error) -> harmos_runtime::operations::FailureKind {
        harmos_runtime::operations::FailureKind::Transient
    }
    async fn run(
        &mut self, context: harmos_runtime::OperationContext, _request: &mut Retry, _: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<String>, Self::Error> {
        let host = context.dependency::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();

        let result: ::core::result::Result<String, Self::Error> = async move {
            assert_eq!(context.context().require::<Principal>().unwrap().0, "alice");
            self.attempts += 1;
            if self.attempts < 3 {
                return Err(std::io::Error::other("retry fixture"));
            }
            Ok(format!("{}:{}", context.context().require::<Principal>().unwrap().0, self.attempts))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
