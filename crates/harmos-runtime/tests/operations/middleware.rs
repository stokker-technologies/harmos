/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{Lanes, app::App};
use harmos_runtime::operations::middleware::{Enter, Invocation, Middleware, Observation};
use harmos_runtime::operations::{CallContext, OperationDeclaration, OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Runtime};

const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
};

struct Principal(&'static str);
#[harmos_runtime::message]
struct Stage {
    id: String,
}
#[harmos_runtime::operation(resources = [harmos_runtime::operations::HostContext], operations = [OverlayChildDeclaration], id = "overlay.parent",  response = String, policy = POLICY)]
struct OverlayParentDeclaration {}

#[harmos_runtime::operation(resources = [harmos_runtime::operations::HostContext], id = "overlay.child",  response = String, policy = POLICY)]
struct OverlayChildDeclaration {}

#[harmos_runtime::operation(resources = [harmos_runtime::operations::HostContext], operations = [DeclareChildDeclaration], id = "middleware.parent",
    response = String,
    policy = POLICY,
    key = keyed
)]
struct DeclareParentDeclaration {}

#[harmos_runtime::operation(resources = [harmos_runtime::operations::HostContext], id = "middleware.child",
    response = String,
    policy = POLICY,
    key = keyed
)]
struct DeclareChildDeclaration {}

struct Policy {
    name: &'static str,
    events: Arc<Mutex<Vec<String>>>,
    reject: bool,
}

impl harmos_runtime::operations::ContextValue for Stage {
    const KEY: &'static str = "test.stage";
}

impl harmos_runtime::Operation for OverlayChildDeclaration {
    type Error = Error;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::Response<String>> {
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let actor = context.context().require::<Principal>()?.0;
        let stage = context.context().value::<Stage>()?.unwrap();
        Ok(format!("{actor}:{}", stage.id).into())
    }
}

impl harmos_runtime::Operation for OverlayParentDeclaration {
    type Error = Error;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::Response<String>> {
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let actor = context.context().require::<Principal>()?.0;
        let stage = context.context().value::<Stage>()?.unwrap();
        assert_eq!(stage.id, "parent");
        let child = context.with_value(Stage { id: "child".into() })?;
        assert_eq!(child.context().require::<Stage>()?.id, "child");
        let answer = child.call::<OverlayChildDeclaration>(OverlayChildDeclaration {}).await?;
        Ok(format!("{actor}:parent:{answer}").into())
    }
}

impl harmos_runtime::Operation for DeclareChildDeclaration {
    type Error = Error;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::Response<String>> {
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        Ok(String::from(context.context().require::<Principal>()?.0).into())
    }
}

impl harmos_runtime::Operation for DeclareParentDeclaration {
    type Error = Error;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> harmos_runtime::Result<harmos_runtime::Response<String>> {
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let actor = context.context().require::<Principal>()?.0;
        let child = context.call::<DeclareChildDeclaration>(DeclareChildDeclaration {}).await?;
        Ok(format!("{actor}:{child}").into())
    }
}

impl Middleware for Policy {
    fn enter<'a>(&'a self, call: &'a mut Invocation) -> Enter<'a> {
        Box::pin(async move {
            self.events.lock().unwrap().push(format!("{} enter {}", self.name, call.operation().id));
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
        self.events.lock().unwrap().push(format!("{} observe {}", self.name, call.operation().id));
    }
}

fn keyed<O>(_: &O) -> Option<harmos_runtime::operations::CacheKey> {
    Some(harmos_runtime::operations::CacheKey::new(vec![1]).unwrap())
}

async fn runtime() -> Runtime<App> {
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    runtime.operations.register::<super::Single<DeclareChildDeclaration>>(Lanes::Open).await.unwrap();
    runtime.operations.register::<super::Single<DeclareParentDeclaration>>(Lanes::Open).await.unwrap();
    runtime
}

fn context(actor: &'static str) -> CallContext {
    let mut context = CallContext::default();
    context.insert(Principal(actor)).unwrap();
    context
}

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
    let values = vec![CallValue {
        key: "test.stage".into(),
        payload: Stage { id: "wire".into() }.encode(),
    }];
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
    call.values = vec![CallValue {
        key: "oversized".into(),
        payload: vec![0; PORTABLE_BYTES],
    }];
    assert!(matches!(call.encode(), Err(harmos_runtime::operations::WireCodecError::Oversized)));
    let wire = original.with_values(decoded.values).unwrap();
    assert!(wire.get::<Stage>().is_none());
    assert_eq!(wire.value::<Stage>().unwrap().unwrap().id, "wire");
    assert!(std::ptr::eq(original.require::<Principal>().unwrap(), wire.require::<Principal>().unwrap()));
    assert!(matches!(original.with_values(vec![values[0].clone(), values[0].clone()]), Err(ContextError::Duplicate { .. })));
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
    runtime.operations.register::<super::Single<OverlayChildDeclaration>>(Lanes::Open).await.unwrap();
    runtime.operations.register::<super::Single<OverlayParentDeclaration>>(Lanes::Open).await.unwrap();
    let mut original = context("alice");
    original.insert_portable(Stage { id: "parent".into() }).unwrap();
    let operations = runtime.operations.with_context(original.clone());
    assert_eq!(&*operations.call::<OverlayParentDeclaration>(OverlayParentDeclaration {}).await.unwrap(), "alice:parent:alice:child");
    assert_eq!(original.require::<Stage>().unwrap().id, "parent");
    assert_eq!(&*operations.call::<OverlayChildDeclaration>(OverlayChildDeclaration {}).await.unwrap(), "alice:parent");
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn children_inherit_principals() {
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
    assert_eq!(&*alice.call::<DeclareParentDeclaration>(DeclareParentDeclaration {}).await.unwrap(), "alice:alice");
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
        alice.call::<DeclareParentDeclaration>(DeclareParentDeclaration {}),
        bob.call::<DeclareParentDeclaration>(DeclareParentDeclaration {})
    );
    assert_eq!(&*a.unwrap(), "alice:alice");
    assert_eq!(&*b.unwrap(), "bob:bob");
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn layers_skips_layers() {
    let runtime = runtime().await;
    let events = Arc::new(Mutex::new(Vec::new()));
    for (name, reject) in [("first", false), ("reject", true), ("unreached", false)] {
        runtime.operations.register_middleware(Arc::new(Policy { name, events: events.clone(), reject })).unwrap();
    }
    assert!(runtime.operations.with_context(context("alice")).call::<DeclareParentDeclaration>(DeclareParentDeclaration {}).await.is_err());
    assert_eq!(
        *events.lock().unwrap(),
        [
            "first enter middleware.parent",
            "reject enter middleware.parent",
            "reject observe middleware.parent",
            "first observe middleware.parent",
        ]
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancellation_interrupts_policy() {
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
    let invocation =
        runtime.operations.with_context(context("alice")).submit::<DeclareParentDeclaration>(DeclareParentDeclaration {}).unwrap();
    (entered.acquire()).await.unwrap().forget();
    invocation.cancel();
    assert!(matches!(invocation.await, Err(Error::OperationRefused(harmos_runtime::Refusal::Cancelled))));
    {
        let observations = observed.lock().unwrap();
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].outcome, harmos_runtime::operations::OperationOutcome::Cancelled);
        assert!(observations[0].settled);
    }
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn overrides_preserve_type() {
    let runtime = runtime().await;
    runtime.operations.unregister(&harmos_runtime::QualifiedId::qualify("host", DeclareChildDeclaration::ID).unwrap()).await.unwrap();
    runtime.operations.register::<super::Single<DeclareChildDeclaration>>(Lanes::Open).await.unwrap();
    let operations = runtime.operations.with_context(context("alice"));
    let result =
        operations.submit_at::<DeclareChildDeclaration>(DeclareChildDeclaration::ID, DeclareChildDeclaration {}).unwrap().await.unwrap();
    assert_eq!(&*result, "alice");
    assert!(operations.submit_at::<DeclareParentDeclaration>(DeclareChildDeclaration::ID, DeclareParentDeclaration {}).is_err());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn retries_preserve_principal() {
    let runtime = runtime().await;
    runtime.operations.register::<super::Single<RetryDeclaration>>(Lanes::Open).await.unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    runtime
        .operations
        .register_middleware(Arc::new(Policy {
            name: "once",
            events: events.clone(),
            reject: false,
        }))
        .unwrap();
    let answer = runtime.operations.with_context(context("alice")).call::<RetryDeclaration>(RetryDeclaration {}).await.unwrap();
    assert_eq!(&*answer, "alice:3");
    assert_eq!(*events.lock().unwrap(), ["once enter middleware.retry", "once observe middleware.retry"]);
    runtime.shutdown().await.unwrap();
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

#[harmos_runtime::operation(resources = [harmos_runtime::operations::HostContext], id = "middleware.retry",
    response = String,
    policy = super::Compute::POLICY
)]
struct RetryDeclaration {}

impl harmos_runtime::Operation for RetryDeclaration {
    type Error = std::io::Error;

    fn classify(_: &Self::Error) -> harmos_runtime::operations::FailureKind {
        harmos_runtime::operations::FailureKind::Transient
    }

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<String>, Self::Error> {
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let principal = context.context().require::<Principal>().unwrap();
        assert_eq!(principal.0, "alice");
        let attempt = context.attempt() + 1;
        if attempt < 3 {
            return Err(std::io::Error::other("retry fixture"));
        }
        Ok(format!("{}:{attempt}", principal.0).into())
    }
}
