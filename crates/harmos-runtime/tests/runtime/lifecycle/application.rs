/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Application Lifecycle And Aggregate Setup Validation.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::Notify;

use harmos_runtime::operations::{Capacity, Lane, LaneSet};
use harmos_runtime::{Application, Lifecycle, Runtime, Setup, Startup, StartupError};

const POLICY: harmos_runtime::OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(1),
    attempt_timeout: Duration::from_secs(1),
    retries: harmos_runtime::RetryPolicy::Never,
};

#[derive(Default)]
struct Counter {
    value: AtomicU64,
    events: Arc<Mutex<Vec<Phase>>>,
    entered: Notify,
    cleanup_attempts: AtomicUsize,
}

#[derive(Clone, Default, serde::Deserialize)]
struct Config {
    #[serde(skip)]
    counter: Arc<Counter>,

    invalid: bool,
    stop_fails: bool,
    cleanup_fails_once: bool,
}

struct App(Config);

#[harmos_runtime::operation(harmos_path = ::harmos_runtime, resources = [Counter], id = "lifecycle.read", response = u64, policy = POLICY)]
struct Read;

#[harmos_runtime::operation(harmos_path = ::harmos_runtime, resources = [Counter], id = "lifecycle.pending", response = (), policy = POLICY)]
struct Pending;

struct PendingGuard(Arc<Counter>);

#[harmos_runtime::operation(harmos_path = ::harmos_runtime, resources = [u64, String], id = "lifecycle.missing", response = (), policy = POLICY)]
struct Missing;

#[derive(Default)]
struct PrepareProbe {
    cleanup: Mutex<Option<harmos_runtime::runtime::BootstrapCleanup>>,
    entered: Notify,
}

#[derive(Default, serde::Deserialize)]
struct PreparingConfig {
    #[serde(skip)]
    probe: Arc<PrepareProbe>,

    pending: bool,
}

struct Preparing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Prepare,
    Initialize,
    Serve,
    Drain,
    Stop,
    Cleanup,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lanes {
    Open,
    Unknown,
}

#[harmos_runtime::catalog(harmos_path = ::harmos_runtime)]
enum Operations {
    Read(Read),
    Pending(Pending),
}

#[harmos_runtime::catalog(harmos_path = ::harmos_runtime)]
enum MissingOperations {
    Missing(Missing),
}

impl Lane for Lanes {
    fn catalog() -> LaneSet {
        LaneSet::declared([("lifecycle.open", Capacity::serial())])
    }
    fn default_lane() -> Self {
        Self::Open
    }
    fn name(&self) -> &'static str {
        match self {
            Self::Open => "lifecycle.open",
            Self::Unknown => "lifecycle.unknown",
        }
    }
    fn capacity(&self) -> Capacity {
        Capacity::serial()
    }
}

impl Application for Preparing {
    type Lanes = Lanes;
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = ();
}

impl Lifecycle for Preparing {
    type Config = PreparingConfig;
    type Error = std::io::Error;
    type Context = Startup<Self>;

    async fn prepare(config: Self::Config, setup: &mut Setup<Self>) -> Result<Self, Self::Error> {
        *config.probe.cleanup.lock().unwrap() = Some(setup.settings().startup_cleanup());
        config.probe.entered.notify_one();
        if config.pending {
            std::future::pending::<()>().await;
        }
        Err(std::io::Error::other("controlled preparation failure"))
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        unreachable!()
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        unreachable!()
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        unreachable!()
    }
}

impl Application for App {
    type Lanes = Lanes;
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = ();
}

impl Lifecycle for App {
    type Config = Config;
    type Error = std::io::Error;
    type Context = Startup<Self>;

    async fn prepare(config: Config, setup: &mut Setup<Self>) -> Result<Self, Self::Error> {
        config.counter.events.lock().unwrap().push(Phase::Prepare);
        setup.resource::<Counter>(config.counter.clone());
        if config.invalid {
            setup.resource::<Counter>(config.counter.clone());
            setup.operations::<MissingOperations>(Lanes::Unknown);
            setup.operations::<MissingOperations>(Lanes::Open);
        } else {
            setup.operations::<Operations>(Lanes::Open);
        }
        Ok(Self(config))
    }

    async fn initialize(&mut self, context: &Self::Context) -> Result<(), Self::Error> {
        context.resource::<Counter>().map_err(std::io::Error::other)?;
        self.0.counter.events.lock().unwrap().push(Phase::Initialize);
        Ok(())
    }

    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        self.0.counter.events.lock().unwrap().push(Phase::Stop);
        if self.0.stop_fails {
            return Err(std::io::Error::other("controlled stop failure"));
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.0.counter.events.lock().unwrap().push(Phase::Cleanup);
        if self.0.cleanup_fails_once && self.0.counter.cleanup_attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(std::io::Error::other("controlled cleanup failure"));
        }
        Ok(())
    }
}

impl harmos_runtime::Operation for Read {
    type Error = std::convert::Infallible;
    async fn run(self, context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u64>, Self::Error> {
        let counter = context.resource::<Counter>();
        counter.events.lock().unwrap().push(Phase::Serve);
        Ok(counter.value.load(Ordering::SeqCst).into())
    }
}

impl harmos_runtime::Operation for Missing {
    type Error = std::convert::Infallible;
    async fn run(self, _: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<()>, Self::Error> {
        Ok(().into())
    }
}

impl harmos_runtime::Operation for Pending {
    type Error = std::convert::Infallible;
    async fn run(self, context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<()>, Self::Error> {
        let counter = context.resource::<Counter>();
        counter.events.lock().unwrap().push(Phase::Serve);
        let guard = PendingGuard(counter.clone());
        counter.entered.notify_one();
        std::future::pending::<()>().await;
        drop(guard);
        Ok(().into())
    }
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        self.0.events.lock().unwrap().push(Phase::Drain);
    }
}

#[tokio::test]
async fn lifecycle_with_overrides() {
    for stop_fails in [false, true] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let real = Arc::new(Counter {
            value: AtomicU64::new(7),
            events: events.clone(),
            entered: Notify::new(),
            cleanup_attempts: AtomicUsize::new(0),
        });
        let fake = Arc::new(Counter {
            value: AtomicU64::new(41),
            events: events.clone(),
            entered: Notify::new(),
            cleanup_attempts: AtomicUsize::new(0),
        });
        let config = Config {
            counter: real,
            invalid: false,
            stop_fails,
            cleanup_fails_once: false,
        };
        let runtime = Runtime::<App>::launch_with((), config, |setup| {
            setup.resource::<Counter>(fake.clone());
        })
        .await
        .unwrap();
        assert!(Arc::ptr_eq(&runtime.operations.context().resource::<Counter>().unwrap(), &fake));
        assert_eq!(*runtime.operation(Read).run().await.unwrap(), 41);
        let pending = runtime.operation(Pending).start().await.unwrap();
        fake.entered.notified().await;
        let (first, second) = tokio::join!(runtime.shutdown(), runtime.shutdown());
        assert_eq!(first.is_err(), stop_fails);
        assert_eq!(second.is_err(), stop_fails);
        assert!(pending.await.is_err());
        assert_eq!(
            *events.lock().unwrap(),
            [Phase::Prepare, Phase::Initialize, Phase::Serve, Phase::Serve, Phase::Drain, Phase::Stop, Phase::Cleanup]
        );
    }
}

#[tokio::test]
async fn setup_errors_aggregate() {
    let counter = Arc::new(Counter::default());
    let config = Config {
        counter: counter.clone(),
        invalid: true,
        stop_fails: false,
        cleanup_fails_once: false,
    };
    let error = match Runtime::<App>::launch((), config).await {
        Ok(_) => panic!("invalid setup cannot launch"),
        Err(error) => error,
    };
    let StartupError::Multiple { errors } = error else { panic!("configuration errors must aggregate") };
    assert_eq!(errors.len(), 5);
    assert_eq!(errors.iter().filter(|error| matches!(error, StartupError::DuplicateResource { .. })).count(), 1);
    assert_eq!(errors.iter().filter(|error| matches!(error, StartupError::DuplicateCatalog { .. })).count(), 1);
    assert_eq!(errors.iter().filter(|error| matches!(error, StartupError::MissingResource { .. })).count(), 2);
    assert_eq!(errors.iter().filter(|error| matches!(error, StartupError::UnknownLane { .. })).count(), 1);
    assert_eq!(*counter.events.lock().unwrap(), [Phase::Prepare, Phase::Cleanup]);
}

#[tokio::test]
async fn invalid_setup_cleanup() {
    let counter = Arc::new(Counter::default());
    let config = Config {
        counter: counter.clone(),
        invalid: true,
        stop_fails: false,
        cleanup_fails_once: true,
    };
    let mut errors = match Runtime::<App>::launch((), config).await {
        Err(StartupError::Multiple { errors }) => errors,
        _ => panic!("setup and cleanup failures must both be reported"),
    };
    let source = errors
        .iter_mut()
        .find_map(|error| match error {
            StartupError::Runtime { source } => source.downcast_mut::<harmos_runtime::runtime::Incompatible>(),
            _ => None,
        })
        .expect("physical cleanup remains retryable");
    assert!(source.cleanup_error().is_some());
    source.retry_cleanup().await.unwrap();
    assert_eq!(*counter.events.lock().unwrap(), [Phase::Prepare, Phase::Cleanup, Phase::Cleanup]);
}

#[tokio::test]
async fn cleanup_retry_once() {
    let counter = Arc::new(Counter::default());
    let config = Config {
        counter: counter.clone(),
        cleanup_fails_once: true,
        ..Config::default()
    };
    let runtime = Runtime::<App>::launch((), config).await.unwrap();
    assert!(runtime.shutdown().await.is_err());
    runtime.shutdown().await.unwrap();
    runtime.shutdown().await.unwrap();
    assert_eq!(*counter.events.lock().unwrap(), [Phase::Prepare, Phase::Initialize, Phase::Stop, Phase::Cleanup, Phase::Cleanup]);
}

#[tokio::test]
async fn lifecycle_reports_failures() {
    let counter = Arc::new(Counter::default());
    let config = Config {
        counter: counter.clone(),
        stop_fails: true,
        cleanup_fails_once: true,
        ..Config::default()
    };
    let runtime = Runtime::<App>::launch((), config).await.unwrap();
    let error = runtime.shutdown().await.unwrap_err();
    let first: &(dyn std::error::Error + 'static) = &error;
    let errors = std::iter::successors(Some(first), |source| source.source());
    let mut typed = errors.filter_map(|source| source.downcast_ref::<harmos_runtime::Error>());
    let combined = typed.find_map(|error| {
        if let harmos_runtime::Error::ApplicationLifecycle { stop, cleanup } = error { Some((stop, cleanup)) } else { None }
    });
    let (stop, cleanup) = combined.expect("both lifecycle failures are retained");
    assert!(matches!(stop.as_ref(), harmos_runtime::Error::TaskFailed { domain: "application stop", .. }));
    assert!(matches!(cleanup.as_ref(), harmos_runtime::Error::TaskFailed { domain: "application cleanup", .. }));

    assert!(runtime.shutdown().await.is_err());
    assert!(runtime.shutdown().await.is_err());
    assert_eq!(*counter.events.lock().unwrap(), [Phase::Prepare, Phase::Initialize, Phase::Stop, Phase::Cleanup, Phase::Cleanup]);
}

#[tokio::test]
async fn prepare_settles_receipt() {
    for pending in [false, true] {
        let probe = Arc::new(PrepareProbe::default());
        let mut startup = Box::pin(Runtime::<Preparing>::launch((), PreparingConfig { probe: probe.clone(), pending }));
        if pending {
            tokio::select! {
                _ = probe.entered.notified() => (),
                _ = &mut startup => panic!("pending prepare cannot finish"),
            }
            drop(startup);
        } else {
            assert!(startup.await.is_err());
        }
        assert_eq!(probe.cleanup.lock().unwrap().as_ref().unwrap().status(), harmos_runtime::runtime::BootstrapStatus::Settled);
    }
}
