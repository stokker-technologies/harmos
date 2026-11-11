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

pub use harmos_storage::execution::{
    Activity, AdvanceError, Clock as Source, Controller, Elapsed, Execution as Clock, Interval, Reading, Sleep,
};

/* === Imports === */

// Standard Library Imports
use std::cell::RefCell;
use std::future::Future;

// Third Party Imports

// Local Imports

/* === Constants === */

thread_local! {
    static CURRENT: RefCell<Option<Clock>> = const { RefCell::new(None) };
}

/* === Macros === */

tokio::task_local! {
    static ACTIVITY: std::sync::Arc<tokio::sync::Mutex<Activity>>;
}

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Entered(Option<Clock>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Drop for Entered {
    fn drop(&mut self) {
        CURRENT.with(|current| *current.borrow_mut() = self.0.take());
    }
}

/* === Public Functions === */

pub(crate) fn current() -> Clock {
    CURRENT.with(|current| current.borrow().clone()).unwrap_or_default()
}

pub(crate) fn enter(clock: Clock) -> Entered {
    Entered(CURRENT.with(|current| current.replace(Some(clock))))
}

pub(crate) async fn scope<F: Future>(clock: Clock, future: F) -> F::Output {
    use crate::supervision::executor::InContext;
    future.in_clock(clock).await
}

pub(crate) async fn active<F: Future>(activity: Activity, future: F) -> F::Output {
    ACTIVITY.scope(std::sync::Arc::new(tokio::sync::Mutex::new(activity)), future).await
}

pub(crate) fn waiting_scope(sleep: Sleep) -> Sleep {
    Box::pin(waiting(sleep))
}

pub(crate) async fn waiting<F: Future>(future: F) -> F::Output {
    let activity = ACTIVITY.try_with(Clone::clone).ok();
    match activity {
        Some(activity) => activity.lock().await.wait(future).await,
        None => future.await,
    }
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    use crate::operations::RegistrationExt;
    use crate::operations::middleware::{AttemptObservation, Enter, Invocation, Middleware};
    use std::sync::{Arc, Mutex};

    pub(crate) struct Model;

    impl crate::Lifecycle for Model {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = crate::Startup<Self>;
        async fn prepare(_: (), _: &mut crate::Setup<Self>) -> Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    impl crate::Application for Model {
        type State = ();
        type Transaction = crate::state::Never;
        type Record = ();
    }

    #[crate::message(harmos_path = crate)]
    #[derive(crate::Request)]
    #[harmos_path(crate)]
    #[request(response = ())]
    #[derive(Clone)]
    struct Wait {}

    static WAIT_DESCRIPTION: crate::operations::WireDeclaration = crate::operations::WireDeclaration::request::<Wait>(
        "test.clock_wait",
        crate::OperationPolicy {
            timeout: Duration::from_secs(60),
            attempt_timeout: Duration::from_secs(60),
            cancellation_timeout: Duration::ZERO,
            retries: crate::RetryPolicy::Never,
        },
    );

    const WAIT_OPERATION: crate::operations::OperationSpec<Wait> = crate::operations::OperationSpec::<Wait>::new(&WAIT_DESCRIPTION);

    async fn wait_handler(
        _request: Wait, context: crate::operations::FunctionContext<Wait>,
    ) -> Result<crate::Response<()>, std::convert::Infallible> {
        let host = context.resource::<crate::operations::HostContext>().expect("declared resource is registered");
        host.borrow().scope().clock().sleep(Duration::from_secs(5)).await;
        Ok(().into())
    }

    fn wait_operation() -> ::core::result::Result<crate::operations::OperationFactory, crate::operations::ServingError> {
        crate::operations::OperationFactory::function(
            WAIT_OPERATION,
            const { &[crate::Resource::of::<crate::operations::HostContext>()] },
            wait_handler,
            |_| crate::operations::FailureKind::Permanent,
        )
    }

    struct Evidence(Arc<Mutex<Vec<AttemptObservation>>>);

    impl Middleware for Evidence {
        fn enter<'a>(&'a self, _: &'a mut Invocation) -> Enter<'a> {
            Box::pin(async { Ok(()) })
        }
        fn attempt<'a>(&'a self, _: &'a Invocation, observation: AttemptObservation) -> Enter<'a> {
            self.0.lock().unwrap().push(observation);
            Box::pin(async { Ok(()) })
        }
    }

    fn clock(seconds: u64) -> Clock {
        Clock::real_at(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
    }

    #[tokio::test(start_paused = true)]
    async fn tasks_retain_their_host_clock_when_polled_together() {
        let first = clock(100);
        let second = clock(200);
        let spawn = || {
            crate::supervision::executor::task(async {
                crate::supervision::executor::sleep(Duration::from_secs(60)).await;
                current().date()
            })
        };
        let first = scope(first, async { Some(spawn()) }).await.unwrap();
        let second = scope(second, async { Some(spawn()) }).await.unwrap();
        let (first, second) = tokio::join!(first, second);
        assert_eq!(first.unwrap(), SystemTime::UNIX_EPOCH + Duration::from_secs(160));
        assert_eq!(second.unwrap(), SystemTime::UNIX_EPOCH + Duration::from_secs(260));
    }

    #[tokio::test(start_paused = true)]
    async fn nested_scopes_restore_the_parent_clock() {
        scope(clock(100), async {
            assert_eq!(current().date(), SystemTime::UNIX_EPOCH + Duration::from_secs(100));
            scope(clock(200), async {
                assert_eq!(current().date(), SystemTime::UNIX_EPOCH + Duration::from_secs(200));
                tokio::task::yield_now().await;
                assert_eq!(current().date(), SystemTime::UNIX_EPOCH + Duration::from_secs(200));
            })
            .await;
            assert_eq!(current().date(), SystemTime::UNIX_EPOCH + Duration::from_secs(100));
        })
        .await;
    }
    #[tokio::test(start_paused = true)]
    async fn records_and_default_attempt_observations_use_the_configured_host_clock() {
        // Configure One Source For The Host
        let clock = clock(123);
        let observations = Arc::new(Mutex::new(Vec::new()));
        let runtime = crate::Runtime::<Model>::launch_with((), (), |setup| {
            setup.settings().clock(clock.clone()).operation_middleware(Arc::new(Evidence(observations.clone())));
        })
        .await
        .unwrap();
        let batch =
            harmos_operations::execution::__private::RegistrationBatch::new().entry(wait_operation().expect("representable operation"));
        runtime.register_batch(batch.lane(crate::operations::Lane::default())).await.unwrap();

        // Record Evidence Before And After A Host Wait
        let fact = crate::state::SystemFact::new(crate::state::FactSource::Work("clock_test".to_owned()), "before".to_owned(), vec![]);
        runtime.records.publish_system(fact).await.unwrap();
        runtime.operation(Wait {}).await.unwrap();
        let fact = crate::state::SystemFact::new(crate::state::FactSource::Work("clock_test".to_owned()), "after".to_owned(), vec![]);
        runtime.records.publish_system(fact).await.unwrap();

        // Dates And Attempt Durations Have The Same Epoch And Elapsed Time
        let dates: Vec<_> = runtime
            .state
            .query::<crate::state::SystemFact>(crate::state::Position::ORIGIN)
            .await
            .map(|entry| entry.unwrap().metadata.at)
            .collect();
        assert_eq!(dates, [SystemTime::UNIX_EPOCH + Duration::from_secs(123), SystemTime::UNIX_EPOCH + Duration::from_secs(128)]);
        let observed = observations.lock().unwrap().clone();
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].started_at.timestamp, dates[0]);
        assert_eq!(observed[0].elapsed, Duration::from_secs(5));
        assert!(runtime.operations.clock().same_source(&clock));
        runtime.shutdown().await.unwrap();
    }
}
