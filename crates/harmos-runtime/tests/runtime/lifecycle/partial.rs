/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Retain Every Partial Startup Owner Through Cleanup Retry

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use crate::runtime::probe::Editor;
use crate::storage::fixture::{Data, Recovery, StorageBackends, StorageManager, config, history};
use harmos_runtime::runtime::{BootstrapCleanup, BootstrapStatus};
use harmos_runtime::services::ServiceContext;
use harmos_runtime::{Application, Lifecycle, Runtime, Setup, Startup, StartupError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Probe {
    cleanup: OnceLock<BootstrapCleanup>,
    allow_cleanup: AtomicBool,
    cleanup_attempts: AtomicUsize,
    stop_attempts: AtomicUsize,
    application_drops: AtomicUsize,
    state_released: Notify,
    block_initialize: AtomicBool,
    initialize_entered: Notify,
}

#[derive(Default, serde::Deserialize)]
struct Config {
    #[serde(skip)]
    probe: Arc<Probe>,
}

struct App(Arc<Probe>);
struct InitializingApp(Arc<Probe>);
struct State(Arc<Probe>);

/* === Enums === */

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
enum Failure {
    #[error("controlled initialization failure")]
    Initialize,

    #[error("controlled stop failure")]
    Stop,
}

/* === Traits === */

/* === Implementations === */

impl Application for App {
    type Lanes = <Editor as Application>::Lanes;
    type State = State;
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = ();
}

impl Application for InitializingApp {
    type Lanes = <Editor as Application>::Lanes;
    type State = State;
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = ();
}

impl Lifecycle for InitializingApp {
    type Config = Config;
    type Error = Failure;
    type Context = Startup<Self>;

    async fn prepare(config: Config, setup: &mut Setup<Self>) -> Result<Self, Self::Error> {
        assert!(config.probe.cleanup.set(setup.settings().startup_cleanup()).is_ok());
        Ok(Self(config.probe))
    }

    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        if self.0.block_initialize.load(Ordering::SeqCst) {
            self.0.initialize_entered.notify_one();
            std::future::pending::<()>().await;
        }
        Err(Failure::Initialize)
    }

    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        self.0.stop_attempts.fetch_add(1, Ordering::SeqCst);
        Err(Failure::Stop)
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.0.cleanup_attempts.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for InitializingApp {
    fn drop(&mut self) {
        self.0.application_drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl Lifecycle for App {
    type Config = Config;
    type Error = std::io::Error;
    type Context = Startup<Self>;

    async fn prepare(config: Config, setup: &mut Setup<Self>) -> Result<Self, Self::Error> {
        assert!(config.probe.cleanup.set(setup.settings().startup_cleanup()).is_ok());
        for _ in 0..2 {
            setup.settings().service("duplicate", |_: ServiceContext<Self>| async { Ok::<(), std::io::Error>(()) });
        }
        Ok(Self(config.probe))
    }

    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        panic!("duplicate services reject startup before initialization")
    }

    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        panic!("an uninitialized application only requires cleanup")
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.0.cleanup_attempts.fetch_add(1, Ordering::SeqCst);
        if !self.0.allow_cleanup.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("controlled cleanup failure"));
        }
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.0.application_drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.0.state_released.notify_one();
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn partial_cleanup_retry() {
    let probe = Arc::new(Probe::default());
    let data = Arc::new(Data::new(history()));
    let recovery = Arc::new(Recovery::new(history()));
    let storage: StorageManager = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: recovery.clone(),
        },
        config(),
    )
    .unwrap();
    let error = Runtime::<App>::launch_with(State(probe.clone()), Config { probe: probe.clone() }, |setup| {
        setup.settings().storage(&storage);
    })
    .await
    .err()
    .expect("duplicate services reject the partial boot");
    assert!(matches!(error, StartupError::Runtime { .. }));
    let receipt = probe.cleanup.get().unwrap();
    assert_eq!(receipt.status(), BootstrapStatus::Failed);
    assert_eq!(probe.cleanup_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(probe.application_drops.load(Ordering::SeqCst), 0);
    assert!(data.control.open.load(Ordering::SeqCst));
    assert!(recovery.control.open.load(Ordering::SeqCst));

    drop(error);
    probe.allow_cleanup.store(true, Ordering::SeqCst);
    receipt.retry().await.unwrap();
    probe.state_released.notified().await;
    assert_eq!(receipt.status(), BootstrapStatus::Settled);
    assert_eq!(probe.cleanup_attempts.load(Ordering::SeqCst), 2);
    assert_eq!(probe.application_drops.load(Ordering::SeqCst), 1);
    assert_eq!(data.control.closes.load(Ordering::SeqCst), 1);
    assert_eq!(recovery.control.closes.load(Ordering::SeqCst), 1);
    assert!(storage.status().closed);
}

#[tokio::test]
async fn logical_failure_settles() {
    let probe = Arc::new(Probe::default());
    let data = Arc::new(Data::new(history()));
    let recovery = Arc::new(Recovery::new(history()));
    let storage: StorageManager = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: recovery.clone(),
        },
        config(),
    )
    .unwrap();
    let error = Runtime::<InitializingApp>::launch_with(State(probe.clone()), Config { probe: probe.clone() }, |setup| {
        setup.settings().storage(&storage);
    })
    .await
    .err()
    .expect("initialization rejects startup");
    let StartupError::Runtime { source } = &error else { panic!("physical startup preserves its typed failure") };
    let physical = source.downcast_ref::<harmos_runtime::runtime::Incompatible>().unwrap();
    let stop = physical.cleanup_error().expect("logical shutdown failure remains reported");
    let failures = std::iter::successors(Some(stop as &(dyn std::error::Error + 'static)), |source| source.source());
    assert!(failures.filter_map(|source| source.downcast_ref::<Failure>()).any(|failure| *failure == Failure::Stop));
    let sources = std::iter::successors(Some(source.as_ref() as &(dyn std::error::Error + 'static)), |source| source.source());
    assert!(sources.filter_map(|source| source.downcast_ref::<Failure>()).any(|failure| *failure == Failure::Initialize));

    let receipt = probe.cleanup.get().unwrap();
    assert_eq!(receipt.status(), BootstrapStatus::Settled);
    assert!(receipt.failure().is_none());
    receipt.retry().await.unwrap();
    probe.state_released.notified().await;
    assert_eq!(probe.stop_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(probe.cleanup_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(probe.application_drops.load(Ordering::SeqCst), 1);
    assert_eq!(data.control.closes.load(Ordering::SeqCst), 1);
    assert_eq!(recovery.control.closes.load(Ordering::SeqCst), 1);
    assert!(storage.status().closed);
}

#[tokio::test]
async fn cancelled_initialization_reports() {
    let probe = Arc::new(Probe::default());
    probe.block_initialize.store(true, Ordering::SeqCst);
    {
        let opening = Runtime::<InitializingApp>::launch(State(probe.clone()), Config { probe: probe.clone() });
        tokio::pin!(opening);
        tokio::select! {
            _ = &mut opening => panic!("initialization is blocked"),
            _ = probe.initialize_entered.notified() => {}
        }
    }
    let receipt = probe.cleanup.get().unwrap();
    let failure = receipt.wait().await.unwrap_err();
    let sources = std::iter::successors(Some(&failure as &(dyn std::error::Error + 'static)), |source| source.source());
    assert!(sources.filter_map(|source| source.downcast_ref::<Failure>()).any(|failure| *failure == Failure::Stop));
    assert_eq!(receipt.status(), BootstrapStatus::Failed);

    receipt.retry().await.unwrap();
    probe.state_released.notified().await;
    assert_eq!(receipt.status(), BootstrapStatus::Settled);
    assert_eq!(probe.stop_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(probe.cleanup_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(probe.application_drops.load(Ordering::SeqCst), 1);
}
