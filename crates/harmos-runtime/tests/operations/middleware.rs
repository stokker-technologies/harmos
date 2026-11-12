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
use super::app::App;
use harmos_runtime::operations::middleware::{Enter, Invocation, Middleware, Observation};
use harmos_runtime::operations::{CallContext, OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Runtime};

/* === Constants === */

const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Principal(&'static str);
#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct Stage {
    id: String,
}
#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = String)]
#[derive(Clone)]
struct OverlayParentDeclaration {}

static OVERLAY_PARENT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<OverlayParentDeclaration>("overlay.parent", POLICY);

static OVERLAY_PARENT_DECLARATION_CALLEES: [harmos_runtime::operations::WireDeclaration; 1] =
    [OVERLAY_CHILD_DECLARATION_DESCRIPTION.copied()];
const OVERLAY_PARENT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<OverlayParentDeclaration> =
    harmos_runtime::operations::OperationSpec::<OverlayParentDeclaration>::new(&OVERLAY_PARENT_DECLARATION_DESCRIPTION)
        .calls(&OVERLAY_PARENT_DECLARATION_CALLEES);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = String)]
#[derive(Clone)]
struct OverlayChildDeclaration {}

static OVERLAY_CHILD_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<OverlayChildDeclaration>("overlay.child", POLICY);

const OVERLAY_CHILD_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<OverlayChildDeclaration> =
    harmos_runtime::operations::OperationSpec::<OverlayChildDeclaration>::new(&OVERLAY_CHILD_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = String)]
#[derive(Clone)]
struct DeclareParentDeclaration {}

static DECLARE_PARENT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<DeclareParentDeclaration>("middleware.parent", POLICY);

static DECLARE_PARENT_DECLARATION_CALLEES: [harmos_runtime::operations::WireDeclaration; 1] =
    [DECLARE_CHILD_DECLARATION_DESCRIPTION.copied()];
const DECLARE_PARENT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<DeclareParentDeclaration> =
    harmos_runtime::operations::OperationSpec::<DeclareParentDeclaration>::new(&DECLARE_PARENT_DECLARATION_DESCRIPTION)
        .calls(&DECLARE_PARENT_DECLARATION_CALLEES);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = String)]
#[derive(Clone)]
struct DeclareChildDeclaration {}

static DECLARE_CHILD_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<DeclareChildDeclaration>("middleware.child", POLICY);

const DECLARE_CHILD_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<DeclareChildDeclaration> =
    harmos_runtime::operations::OperationSpec::<DeclareChildDeclaration>::new(&DECLARE_CHILD_DECLARATION_DESCRIPTION);

struct Policy {
    name: &'static str,
    events: Arc<Mutex<Vec<String>>>,
    reject: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::operations::ContextValue for Stage {
    const KEY: &'static str = "test.stage";
}

async fn overlay_child_declaration_handler(
    _request: OverlayChildDeclaration, ctx: harmos_runtime::operations::FunctionContext<OverlayChildDeclaration>,
) -> harmos_runtime::Result<harmos_runtime::Response<String>> {
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let actor = context.context().require::<Principal>()?.0;
    let stage = context.context().value::<Stage>()?.unwrap();
    Ok(format!("{actor}:{}", stage.id).into())
}

fn overlay_child_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        OVERLAY_CHILD_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        overlay_child_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

async fn overlay_parent_declaration_handler(
    _request: OverlayParentDeclaration, ctx: harmos_runtime::operations::FunctionContext<OverlayParentDeclaration>,
) -> harmos_runtime::Result<harmos_runtime::Response<String>> {
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let actor = context.context().require::<Principal>()?.0;
    let stage = context.context().value::<Stage>()?.unwrap();
    assert_eq!(stage.id, "parent");
    let child = context.with_value(Stage { id: "child".into() })?;
    assert_eq!(child.context().require::<Stage>()?.id, "child");
    let answer = child.operation::<OverlayChildDeclaration>(OverlayChildDeclaration {}).await?;
    Ok(format!("{actor}:parent:{answer}").into())
}

fn overlay_parent_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        OVERLAY_PARENT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        overlay_parent_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

async fn declare_child_declaration_handler(
    _request: DeclareChildDeclaration, ctx: harmos_runtime::operations::FunctionContext<DeclareChildDeclaration>,
) -> harmos_runtime::Result<harmos_runtime::Response<String>> {
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    Ok(String::from(context.context().require::<Principal>()?.0).into())
}

fn declare_child_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        DECLARE_CHILD_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        declare_child_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

async fn declare_parent_declaration_handler(
    _request: DeclareParentDeclaration, ctx: harmos_runtime::operations::FunctionContext<DeclareParentDeclaration>,
) -> harmos_runtime::Result<harmos_runtime::Response<String>> {
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let actor = context.context().require::<Principal>()?.0;
    let child = context.operation::<DeclareChildDeclaration>(DeclareChildDeclaration {}).await?;
    Ok(format!("{actor}:{child}").into())
}

fn declare_parent_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        DECLARE_PARENT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        declare_parent_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
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

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> Runtime<App> {
    let runtime = Runtime::<App>::launch((), ()).await.unwrap();
    runtime
        .operations
        .register_operation(
            declare_child_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        )
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(
            declare_parent_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        )
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
    let values = vec![CallValue {
        key: "test.stage".into(),
        payload: Stage { id: "wire".into() }.encode(),
    }];
    let wire = original.with_values(values.clone()).unwrap();
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
    runtime
        .operations
        .register_operation(
            overlay_child_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        )
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(
            overlay_parent_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        )
        .await
        .unwrap();
    let mut original = context("alice");
    original.insert_portable(Stage { id: "parent".into() }).unwrap();
    let operations = runtime.operations.with_context(original.clone());
    assert_eq!(&*operations.operation::<OverlayParentDeclaration>(OverlayParentDeclaration {}).await.unwrap(), "alice:parent:alice:child");
    assert_eq!(original.require::<Stage>().unwrap().id, "parent");
    assert_eq!(&*operations.operation::<OverlayChildDeclaration>(OverlayChildDeclaration {}).await.unwrap(), "alice:parent");
    crate::support::app::shutdown(&runtime).await.unwrap();
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
    assert_eq!(&*alice.operation::<DeclareParentDeclaration>(DeclareParentDeclaration {}).await.unwrap(), "alice:alice");
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
        alice.operation::<DeclareParentDeclaration>(DeclareParentDeclaration {}),
        bob.operation::<DeclareParentDeclaration>(DeclareParentDeclaration {})
    );
    assert_eq!(&*a.unwrap(), "alice:alice");
    assert_eq!(&*b.unwrap(), "bob:bob");
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn layers_skips_layers() {
    let runtime = runtime().await;
    let events = Arc::new(Mutex::new(Vec::new()));
    for (name, reject) in [("first", false), ("reject", true), ("unreached", false)] {
        runtime.operations.register_middleware(Arc::new(Policy { name, events: events.clone(), reject })).unwrap();
    }
    assert!(
        runtime.operations.with_context(context("alice")).operation::<DeclareParentDeclaration>(DeclareParentDeclaration {}).await.is_err()
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
    crate::support::app::shutdown(&runtime).await.unwrap();
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
    invocation.request_cancel();
    assert!(matches!(invocation.await, Err(Error::OperationRefused(harmos_runtime::Refusal::Cancelled { .. }))));
    {
        let observations = observed.lock().unwrap();
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].outcome, harmos_runtime::operations::OperationOutcome::Cancelled);
        assert!(observations[0].settled);
    }
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn overrides_preserve_type() {
    let runtime = runtime().await;
    runtime
        .operations
        .unregister(&harmos_runtime::QualifiedId::qualify("host", DECLARE_CHILD_DECLARATION_OPERATION.entry().id).unwrap())
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(
            declare_child_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        )
        .await
        .unwrap();
    let operations = runtime.operations.with_context(context("alice"));
    let result = operations
        .submit_at::<DeclareChildDeclaration>(DECLARE_CHILD_DECLARATION_OPERATION.entry().id, DeclareChildDeclaration {})
        .unwrap()
        .await
        .unwrap();
    assert_eq!(&*result, "alice");
    assert!(
        operations
            .submit_at::<DeclareParentDeclaration>(DECLARE_CHILD_DECLARATION_OPERATION.entry().id, DeclareParentDeclaration {})
            .is_err()
    );
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn retries_preserve_principal() {
    let runtime = runtime().await;
    runtime
        .operations
        .register_operation(retry_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default())
        .await
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
    let answer = runtime.operations.with_context(context("alice")).operation::<RetryDeclaration>(RetryDeclaration {}).await.unwrap();
    assert_eq!(&*answer, "alice:3");
    assert_eq!(*events.lock().unwrap(), ["once enter middleware.retry", "once observe middleware.retry"]);
    crate::support::app::shutdown(&runtime).await.unwrap();
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

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = String)]
#[derive(Clone)]
struct RetryDeclaration {}

static RETRY_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<RetryDeclaration>("middleware.retry", super::COMPUTE_OPERATION.entry().policy);

const RETRY_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<RetryDeclaration> =
    harmos_runtime::operations::OperationSpec::<RetryDeclaration>::new(&RETRY_DECLARATION_DESCRIPTION);

async fn retry_declaration_handler(
    _request: RetryDeclaration, ctx: harmos_runtime::operations::FunctionContext<RetryDeclaration>,
) -> Result<harmos_runtime::Response<String>, std::io::Error> {
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let principal = context.context().require::<Principal>().unwrap();
    assert_eq!(principal.0, "alice");
    let attempt = context.attempt() + 1;
    if attempt < 3 {
        return Err(std::io::Error::other("retry fixture"));
    }
    Ok(format!("{}:{attempt}", principal.0).into())
}

fn retry_declaration_classification(_: &std::io::Error) -> harmos_runtime::operations::FailureKind {
    harmos_runtime::operations::FailureKind::Transient
}

fn retry_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        RETRY_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        retry_declaration_handler,
        retry_declaration_classification,
    )
}
