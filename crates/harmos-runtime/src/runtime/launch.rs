/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Application Preparation And Retained Lifecycle Ownership.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};

// Third Party Imports
use futures_util::FutureExt;

// Local Imports
use super::{Builder, Runtime, StartupError};
use crate::operations::{Lane, LaneSettings};
use crate::supervision::cleanup::{Cleanup, Finalize, Resource};
use crate::{Application, Error, Setup, Startup};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Cause = Box<dyn std::error::Error + Send + Sync>;

/* === Structs === */

pub(super) struct ApplicationCleanup<A: Application> {
    owner: Arc<Cleanup<Owner<A>>>,
    failure: Arc<Mutex<Option<Arc<Error>>>>,
}

struct Preparation {
    cleanup: super::BootstrapCleanup,
    armed: bool,
}

struct Owner<A: Application> {
    application: A,
    context: Option<Startup<A>>,
    initialized: bool,
    stop_attempted: bool,
    failure: Arc<Mutex<Option<Arc<Error>>>>,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
enum LifecycleFailure {
    #[error("application {phase} failed: {source}")]
    Callback {
        phase: &'static str,

        #[source]
        source: Cause,
    },

    #[error("application {phase} panicked")]
    Panicked { phase: &'static str },
}

/* === Traits === */

/* === Implementations === */

impl Drop for Preparation {
    fn drop(&mut self) {
        if self.armed {
            self.cleanup.settle_unowned();
        }
    }
}

impl<A: Application> Runtime<A> {
    pub async fn launch(origin: impl Into<crate::state::Origin<A::State>>, config: A::Config) -> Result<Self, StartupError> {
        let launch = Self::launch_with(origin, config, |_| {});
        Box::pin(launch).await
    }

    pub async fn launch_with(
        origin: impl Into<crate::state::Origin<A::State>>, config: A::Config, overrides: impl FnOnce(&mut Setup<A>),
    ) -> Result<Self, StartupError> {
        let mut setup = Setup::<A>::new(Self::builder(origin));
        let mut preparation = Preparation {
            cleanup: setup.settings().startup_cleanup(),
            armed: true,
        };
        let prepared = guarded("prepare", A::prepare(config, &mut setup)).await;
        if prepared.is_ok() {
            preparation.armed = false;
        }
        setup.overrides(overrides);
        let validation = setup.validate();
        let mut errors = Vec::new();
        if let Err(error) = validation {
            errors.push(error);
        }
        let application = match prepared {
            Ok(application) => application,
            Err(source) => {
                errors.push(StartupError::Runtime { source });
                return Err(StartupError::aggregate(errors).expect("prepare failed"));
            }
        };
        if let Some(error) = StartupError::aggregate(errors) {
            let receipt = setup.settings().startup_cleanup();
            let mut rejected = super::boot::Boot::new(None, receipt).expect("no storage claim before setup validation");
            rejected.application = Some(ApplicationCleanup::new(application, None));
            let rejection = rejected.reject(super::Incompatible::work(&error)).await;
            if rejection.cleanup.is_some() {
                return Err(StartupError::Multiple {
                    errors: vec![error, StartupError::Runtime { source: Box::new(rejection) }],
                });
            }
            return Err(error);
        }
        let mut builder = setup.into_configuration();
        builder.application = Some(application);
        builder.runtime().await
    }
}

impl<A: Application> harmos_sdk::lifecycle::SetupConfiguration for Builder<A> {
    fn resource<T: Send + Sync + 'static>(&mut self, value: Arc<T>, replace: bool) {
        if replace {
            self.resources.replace(value);
            return;
        }
        self.resource::<T>(value);
    }

    fn validate(&mut self) -> Result<(), StartupError> {
        self.validate_configuration()
    }
}

impl<A: Application> harmos_sdk::lifecycle::ApplicationConfiguration for Builder<A> {
    type Lane = A::Lanes;
    type LaneSettings = LaneSettings;

    fn lane(&mut self, lane: A::Lanes, settings: LaneSettings) {
        if !self.lanes.configure(lane.name(), settings.capacity) {
            self.configuration_errors.push(StartupError::UnknownLane { lane: lane.name() });
        }
    }

    fn operations<C: harmos_sdk::operations::OperationCatalog>(&mut self, lane: A::Lanes) {
        self.operations::<C>(lane);
    }
}

impl<A: Application> Clone for ApplicationCleanup<A> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            failure: self.failure.clone(),
        }
    }
}

impl<A: Application> ApplicationCleanup<A> {
    pub(super) fn new(application: A, context: Option<Startup<A>>) -> Self {
        let failure = Arc::new(Mutex::new(None));
        let owner = Owner {
            application,
            context,
            initialized: false,
            stop_attempted: false,
            failure: failure.clone(),
        };
        Self {
            owner: Arc::new(Cleanup::new(owner)),
            failure,
        }
    }

    pub(super) async fn initialize(&self, context: Startup<A>) -> Result<(), Cause> {
        let mut original = self.owner.take().expect("initialization owns its application");
        let owner = original.resource();
        owner.context = Some(context);
        owner.initialized = true;
        let context = owner.context.as_ref().expect("runtime startup supplies its context");
        guarded("initialize", owner.application.initialize(context)).await
    }

    pub(super) async fn settle(&self) -> Result<(), Arc<Error>> {
        self.owner.settle().await
    }

    pub(super) fn failure(&self) -> Option<Arc<Error>> {
        self.failure.lock().expect("application lifecycle outcome is healthy").clone()
    }
}

impl<A: Application> Resource for Owner<A> {
    fn finalize(&mut self) -> Finalize<'_> {
        Box::pin(async move {
            if self.initialized && !self.stop_attempted {
                self.stop_attempted = true;
                let context = self.context.as_ref().expect("initialized application retains its context");
                if let Err(source) = guarded("stop", self.application.stop(context)).await {
                    let error = Error::TaskFailed {
                        domain: "application stop",
                        source: Arc::from(source),
                    };
                    *self.failure.lock().expect("application lifecycle outcome is healthy") = Some(Arc::new(error));
                }
            }
            let cleanup = guarded("cleanup", self.application.cleanup()).await.map_err(|source| Error::TaskFailed {
                domain: "application cleanup",
                source: Arc::from(source),
            });
            let Err(cleanup) = cleanup else { return Ok(()) };
            let stop = self.failure.lock().expect("application lifecycle outcome is healthy").clone();
            let error = match stop {
                Some(stop) => Error::ApplicationLifecycle { stop, cleanup: Arc::new(cleanup) },
                None => cleanup,
            };
            Err(Arc::new(error))
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn guarded<T, E: std::error::Error + Send + Sync + 'static>(
    phase: &'static str, future: impl Future<Output = Result<T, E>>,
) -> Result<T, Cause> {
    match AssertUnwindSafe(future).catch_unwind().await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(source)) => Err(Box::new(LifecycleFailure::Callback { phase, source: Box::new(source) })),
        Err(_) => Err(Box::new(LifecycleFailure::Panicked { phase })),
    }
}

/* === Tests === */
