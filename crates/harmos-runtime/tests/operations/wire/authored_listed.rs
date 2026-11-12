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
use std::sync::Arc;
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::app::App;
use harmos_runtime::operations::{OperationPolicy, RetryPolicy};

/* === Constants === */

const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(1),
    attempt_timeout: Duration::from_secs(1),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u32)]
#[derive(Clone)]
pub struct DoublingDeclaration {
    value: u32,
}

pub static DOUBLING_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<DoublingDeclaration>("authored.doubling", POLICY);

pub const DOUBLING_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<DoublingDeclaration> =
    harmos_runtime::operations::OperationSpec::<DoublingDeclaration>::new(&DOUBLING_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u32)]
#[derive(Clone)]
struct InferredOwnedDeclaration {
    value: u32,
}

static INFERRED_OWNED_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<InferredOwnedDeclaration>("authored.inferred_owned", POLICY);

const INFERRED_OWNED_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<InferredOwnedDeclaration> =
    harmos_runtime::operations::OperationSpec::<InferredOwnedDeclaration>::new(&INFERRED_OWNED_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u32)]
#[derive(Clone)]
struct BorrowOwnerDeclaration {
    value: u32,
}

static BORROW_OWNER_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<BorrowOwnerDeclaration>("authored.borrow_owner", POLICY);

const BORROW_OWNER_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<BorrowOwnerDeclaration> =
    harmos_runtime::operations::OperationSpec::<BorrowOwnerDeclaration>::new(&BORROW_OWNER_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u32)]
#[derive(Clone)]
struct DeclaredOwnedDeclaration {
    value: u32,
}

static DECLARED_OWNED_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<DeclaredOwnedDeclaration>("authored.declared_owned", POLICY);

const DECLARED_OWNED_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<DeclaredOwnedDeclaration> =
    harmos_runtime::operations::OperationSpec::<DeclaredOwnedDeclaration>::new(&DECLARED_OWNED_DECLARATION_DESCRIPTION);

struct BorrowOwner {
    value: u32,
    borrowed: std::sync::atomic::AtomicUsize,
    ran: std::sync::atomic::AtomicUsize,
    operations: std::sync::OnceLock<std::sync::Weak<harmos_runtime::operations::OperationManager>>,
}

/* === Enums === */

pub fn declared_catalog() -> harmos_runtime::operations::Catalog {
    declared_catalog_batch().named(concat!(module_path!(), "::Declared"))
}

fn declared_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(doubling_declaration_operation().expect("representable operation"));
    batch
}

/* === Traits === */

/* === Implementations === */

pub async fn doubling_declaration_handler(
    request: DoublingDeclaration, _: harmos_runtime::operations::FunctionContext<DoublingDeclaration>,
) -> Result<harmos_runtime::Response<u32>, harmos_runtime::Error> {
    Ok((request.value * 2).into())
}

pub fn doubling_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        DOUBLING_DECLARATION_OPERATION,
        const { &[] },
        doubling_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

async fn inferred_owned_declaration_handler(
    _request: InferredOwnedDeclaration, ctx: harmos_runtime::operations::FunctionContext<InferredOwnedDeclaration>,
) -> Result<harmos_runtime::Response<u32>, harmos_runtime::Error> {
    Ok(harmos_runtime::Response::from_owner(ctx.resource::<u32>().expect("declared resource is registered")))
}

fn inferred_owned_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        INFERRED_OWNED_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<u32>()] },
        inferred_owned_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

async fn declared_owned_declaration_handler(
    _request: DeclaredOwnedDeclaration, ctx: harmos_runtime::operations::FunctionContext<DeclaredOwnedDeclaration>,
) -> Result<harmos_runtime::Response<u32>, harmos_runtime::Error> {
    Ok(harmos_runtime::Response::from_owner(ctx.resource::<u32>().expect("declared resource is registered")))
}

fn declared_owned_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        DECLARED_OWNED_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<u32>()] },
        declared_owned_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl std::borrow::Borrow<u32> for BorrowOwner {
    fn borrow(&self) -> &u32 {
        self.borrowed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.operations.get().unwrap().upgrade().unwrap().definition("authored.borrow_owner").unwrap();
        &self.value
    }
}

async fn borrow_owner_declaration_handler(
    _request: BorrowOwnerDeclaration, ctx: harmos_runtime::operations::FunctionContext<BorrowOwnerDeclaration>,
) -> Result<harmos_runtime::Response<u32>, harmos_runtime::Error> {
    let owner = ctx.resource::<BorrowOwner>().expect("declared resource is registered");
    owner.ran.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    Ok(harmos_runtime::Response::from_owner(owner))
}

fn borrow_owner_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        BORROW_OWNER_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<BorrowOwner>()] },
        borrow_owner_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn restore_defers_borrow() {
    let owner = Arc::new(BorrowOwner {
        value: 7,
        borrowed: std::sync::atomic::AtomicUsize::new(0),
        ran: std::sync::atomic::AtomicUsize::new(0),
        operations: std::sync::OnceLock::new(),
    });
    let runtime = harmos_runtime::Runtime::<App>::launch_with((), (), |setup| {
        setup
            .resource::<BorrowOwner>(owner.clone())
            .operation(borrow_owner_declaration_operation().expect("representable operation"), harmos_runtime::operations::Lane::default());
    })
    .await
    .unwrap();
    let observed = Arc::new(runtime.operations.clone());
    owner.operations.set(Arc::downgrade(&observed)).unwrap();
    let first = runtime.operation(BorrowOwnerDeclaration { value: 0 }).await.unwrap();
    let second = runtime.operation(BorrowOwnerDeclaration { value: 0 }).await.unwrap();
    assert!(first.ptr_eq(&second));
    assert_eq!(owner.ran.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(owner.borrowed.load(std::sync::atomic::Ordering::SeqCst), 0);
    let erased = second.clone().into_erased();
    assert!(erased.clone().restore::<String>().is_none());
    let restored = erased.restore::<u32>().unwrap();
    assert_eq!(owner.borrowed.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(*restored, 7);
    assert_eq!(owner.borrowed.load(std::sync::atomic::Ordering::SeqCst), 1);
    drop((first, second, restored));
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn macro_retains_owner() {
    let original = Arc::new(7_u32);
    let runtime = harmos_runtime::Runtime::<App>::launch_with((), (), |setup| {
        setup
            .resource::<u32>(original.clone())
            .operation(
                inferred_owned_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default(),
            )
            .operation(
                declared_owned_declaration_operation().expect("representable operation"),
                harmos_runtime::operations::Lane::default(),
            );
    })
    .await
    .unwrap();
    let inferred = runtime.operation(InferredOwnedDeclaration { value: 0 }).await.unwrap();
    let declared = runtime.operation(DeclaredOwnedDeclaration { value: 0 }).await.unwrap();
    assert_eq!(*inferred, 7_u32);
    assert_eq!(*declared, 7_u32);
    assert!(std::ptr::eq(inferred.as_ref(), original.as_ref()));
    assert!(inferred.ptr_eq(&declared));
    drop((inferred, declared));
    crate::support::app::shutdown(&runtime).await.unwrap();
    drop(runtime);
    let recovered = harmos_runtime::operations::Response::from_owner(original).try_unwrap().unwrap();
    assert_eq!(recovered, 7);
}

/* === Tests === */

#[test]
fn catalogs_project_declarations() {
    let projected = declared_catalog().definitions().expect("valid catalog");
    assert_eq!(projected[0].id.as_str(), "authored.doubling");
    let contracts = declared_catalog().contracts().unwrap();
    assert_eq!(contracts[0].fingerprint(), projected[0].fingerprint);
}
