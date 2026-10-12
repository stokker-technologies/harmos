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
pub use harmos_contract::observers::{Exit as ObserverExit, State as ObserverStatus};

/* === Imports === */
// Standard Library Imports
use std::sync::{Arc, Mutex};

// Third Party Imports
use tokio::sync::{Notify, watch};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(super) struct Lifecycle {
    control: Arc<Control>,
    finished: bool,
}

pub(super) struct Control {
    pub(super) state: watch::Sender<ObserverStatus>,
    failure: Mutex<Option<Arc<Failure>>>,
    retry: Notify,
}

#[derive(Clone, Debug)]
pub(super) struct Failure {
    pub(super) exit: ObserverExit,
    pub(super) source: Option<Arc<dyn std::error::Error + Send + Sync>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Lifecycle {
    pub(super) fn new() -> (Self, Arc<Control>) {
        let control = Arc::new(Control {
            state: watch::channel(ObserverStatus::Starting).0,
            failure: Mutex::new(None),
            retry: Notify::new(),
        });
        (
            Self {
                control: control.clone(),
                finished: false,
            },
            control,
        )
    }

    pub(super) fn finalizing(&self) {
        self.control.state.send_replace(ObserverStatus::Finalizing);
    }

    pub(super) async fn retained(&self, failure: Failure) {
        let exit = failure.exit;
        *self
            .control
            .failure
            .lock()
            .expect("observer failure is healthy") = Some(Arc::new(failure));
        self.control
            .state
            .send_replace(ObserverStatus::Retained(exit));
        self.control.retry.notified().await;
    }

    pub(super) fn running(&self) {
        self.control.state.send_if_modified(|state| {
            if *state != ObserverStatus::Starting {
                return false;
            }
            *state = ObserverStatus::Running;
            true
        });
    }

    pub(super) fn stopping(&self) {
        stopping(&self.control.state);
    }

    pub(super) fn finish(mut self, exit: ObserverExit) {
        self.control
            .state
            .send_replace(ObserverStatus::Stopped(exit));
        self.finished = true;
    }
}

impl Drop for Lifecycle {
    fn drop(&mut self) {
        if !self.finished {
            self.control
                .state
                .send_replace(ObserverStatus::Stopped(ObserverExit::Interrupted));
        }
    }
}

impl Control {
    pub(super) fn failure(&self) -> Option<std::io::Error> {
        if !matches!(*self.state.borrow(), ObserverStatus::Retained(_)) {
            return None;
        }
        self.failure
            .lock()
            .expect("observer failure is healthy")
            .clone()
            .map(std::io::Error::other)
    }

    pub(super) fn recover(&self) {
        let requested = self.state.send_if_modified(|state| {
            let ObserverStatus::Retained(exit) = *state else {
                return false;
            };
            self.failure
                .lock()
                .expect("observer failure is healthy")
                .take();
            *state = match exit {
                ObserverExit::FinalizeFailed | ObserverExit::FinalizePanicked => {
                    ObserverStatus::Finalizing
                }
                _ => ObserverStatus::Stopping,
            };
            true
        });
        if requested {
            self.retry.notify_one();
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "observer cleanup retained: {:?}", self.exit)?;
        if let Some(source) = &self.source {
            write!(formatter, ": {source}")?;
        }
        Ok(())
    }
}
impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|source| source.as_ref() as _)
    }
}

/* === Public Functions === */
pub(super) fn stopping(state: &watch::Sender<ObserverStatus>) {
    state.send_if_modified(|state| {
        if matches!(
            state,
            ObserverStatus::Stopping
                | ObserverStatus::Finalizing
                | ObserverStatus::Retained(_)
                | ObserverStatus::Stopped(_)
        ) {
            return false;
        }
        *state = ObserverStatus::Stopping;
        true
    });
}

/* === Private Functions === */

/* === Tests === */
