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
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::{Error, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Trace {
    entered: AtomicUsize,
    dropped: AtomicUsize,
    running: Notify,
}

struct AttemptGuard(Arc<Trace>);

#[harmos_runtime::message]
struct Answer {
    value: u64,
}

#[harmos_runtime::operation(resources = [Trace], id = "authored.read", response = Answer,
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        retries: harmos_runtime::RetryPolicy::Never,
    },
)]
struct ReadContractDeclaration {
    mode: Mode,
}

/* === Enums === */

#[harmos_runtime::message]
enum Mode {
    Complete,
    Wait,
    Panic,
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for ReadContractDeclaration {
    type Error = std::convert::Infallible;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Answer>, Self::Error> {
        let trace = ctx.resource::<Trace>();
        let _guard = AttemptGuard(trace.clone());
        trace.entered.fetch_add(1, Ordering::SeqCst);
        trace.running.notify_one();
        match self.mode {
            Mode::Complete => Ok(Answer { value: 7 }.into()),
            Mode::Wait => std::future::pending().await,
            Mode::Panic => panic!("scripted attempt panic"),
        }
    }
}

impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Trace>) {
    let trace = Arc::new(Trace::default());
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Trace>(trace.clone()).operations::<crate::operations::Single<ReadContractDeclaration>>(Lanes::Open);
    })
    .await
    .unwrap();
    (runtime, trace)
}

/* === Tests === */

#[tokio::test]
async fn registration_does_not_execute_requests() {
    let (runtime, trace) = runtime().await;
    assert_eq!(trace.entered.load(Ordering::SeqCst), 0);
    let response = runtime.operation(ReadContractDeclaration { mode: Mode::Complete }).run().await.unwrap();
    assert_eq!(response.value, 7);
    assert_eq!(trace.entered.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancellation_drops_attempt_state() {
    let (runtime, trace) = runtime().await;
    let call = runtime.operation(ReadContractDeclaration { mode: Mode::Wait }).start().await.unwrap();
    trace.running.notified().await;
    call.cancel();
    assert!(call.await.is_err());
    assert_eq!(trace.entered.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn panic_drops_attempt_state() {
    let (runtime, trace) = runtime().await;
    let result = runtime.operation(ReadContractDeclaration { mode: Mode::Panic }).run().await;
    assert!(matches!(result, Err(Error::OperationPanicked { .. })));
    assert_eq!(trace.entered.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

mod listed {

    use super::App;
    use std::sync::Arc;
    use std::time::Duration;

    use harmos_runtime::operations::{Contracts, Declarations, OperationPolicy, RetryPolicy};

    const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(1),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::Never,
    };

    #[harmos_runtime::operation(id = "authored.doubling",  response = u32, policy = POLICY)]
    pub struct DoublingDeclaration {
        value: u32,
    }

    #[harmos_runtime::operation(resources = [u32], id = "authored.inferred_owned",
        response = u32,
        policy = POLICY
    )]
    struct InferredOwnedDeclaration {
        value: u32,
    }

    #[harmos_runtime::operation(resources = [BorrowOwner], id = "authored.cached_owner",
        response = u32,
        policy = POLICY,
        key = owner_key
    )]
    struct CachedOwnerDeclaration {
        value: u32,
    }

    #[harmos_runtime::operation(resources = [u32], id = "authored.declared_owned", response = u32, policy = POLICY)]
    struct DeclaredOwnedDeclaration {
        value: u32,
    }

    impl harmos_runtime::Operation for DoublingDeclaration {
        type Error = harmos_runtime::Error;

        async fn run(self, _: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u32>, Self::Error> {
            Ok((self.value * 2).into())
        }
    }

    impl harmos_runtime::Operation for InferredOwnedDeclaration {
        type Error = harmos_runtime::Error;

        async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u32>, Self::Error> {
            Ok(harmos_runtime::Response::from_owner(ctx.resource::<u32>()))
        }
    }

    impl harmos_runtime::Operation for DeclaredOwnedDeclaration {
        type Error = harmos_runtime::Error;

        async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u32>, Self::Error> {
            Ok(harmos_runtime::Response::from_owner(ctx.resource::<u32>()))
        }
    }

    struct BorrowOwner {
        value: u32,
        borrowed: std::sync::atomic::AtomicUsize,
        ran: std::sync::atomic::AtomicUsize,
        operations: std::sync::OnceLock<std::sync::Weak<harmos_runtime::operations::OperationManager<crate::operations::fixture::Lanes>>>,
    }

    impl std::borrow::Borrow<u32> for BorrowOwner {
        fn borrow(&self) -> &u32 {
            self.borrowed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.operations.get().unwrap().upgrade().unwrap().definition("authored.cached_owner").unwrap();
            &self.value
        }
    }

    fn owner_key(request: &CachedOwnerDeclaration) -> Option<harmos_runtime::operations::CacheKey> {
        Some(harmos_runtime::operations::CacheKey::new(request.value.to_le_bytes().to_vec()).unwrap())
    }

    impl harmos_runtime::Operation for CachedOwnerDeclaration {
        type Error = harmos_runtime::Error;

        async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u32>, Self::Error> {
            let owner = ctx.resource::<BorrowOwner>();
            owner.ran.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(harmos_runtime::Response::from_owner(owner))
        }
    }

    #[harmos_runtime::catalog]
    pub enum Declared {
        Doubling(DoublingDeclaration),
    }

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
                .operations::<crate::operations::Single<CachedOwnerDeclaration>>(crate::operations::fixture::Lanes::Open);
        })
        .await
        .unwrap();
        let observed = Arc::new(runtime.operations.clone());
        owner.operations.set(Arc::downgrade(&observed)).unwrap();
        let first = runtime.operation(CachedOwnerDeclaration { value: 0 }).run().await.unwrap();
        let cached = runtime.operation(CachedOwnerDeclaration { value: 0 }).run().await.unwrap();
        assert!(first.ptr_eq(&cached));
        assert_eq!(owner.ran.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(owner.borrowed.load(std::sync::atomic::Ordering::SeqCst), 0);
        let erased = cached.clone().into_erased();
        assert!(erased.clone().restore::<String>().is_none());
        let restored = erased.restore::<u32>().unwrap();
        assert_eq!(owner.borrowed.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(*restored, 7);
        assert_eq!(owner.borrowed.load(std::sync::atomic::Ordering::SeqCst), 1);
        drop((first, cached, restored));
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn macro_retains_owner() {
        let original = Arc::new(7_u32);
        let runtime = harmos_runtime::Runtime::<App>::launch_with((), (), |setup| {
            setup
                .resource::<u32>(original.clone())
                .operations::<crate::operations::Single<InferredOwnedDeclaration>>(crate::operations::fixture::Lanes::Open)
                .operations::<crate::operations::Single<DeclaredOwnedDeclaration>>(crate::operations::fixture::Lanes::Open);
        })
        .await
        .unwrap();
        let inferred = runtime.operation(InferredOwnedDeclaration { value: 0 }).run().await.unwrap();
        let declared = runtime.operation(DeclaredOwnedDeclaration { value: 0 }).run().await.unwrap();
        assert_eq!(*inferred, 7_u32);
        assert_eq!(*declared, 7_u32);
        assert!(std::ptr::eq(inferred.as_ref(), original.as_ref()));
        assert!(inferred.ptr_eq(&declared));
        drop((inferred, declared));
        runtime.shutdown().await.unwrap();
        drop(runtime);
        let recovered = harmos_runtime::operations::Response::from_owner(original).try_unwrap().unwrap();
        assert_eq!(recovered, 7);
    }

    #[test]
    fn catalogs_project_declarations() {
        let projected = Declared::catalog();
        assert_eq!(projected, Declarations::of::<Declared>().unwrap());
        assert_eq!(projected[0].id.as_str(), "authored.doubling");
        let contracts = Contracts::of::<Declared>().unwrap();
        assert_eq!(contracts[0].fingerprint(), projected[0].fingerprint);
    }
}
