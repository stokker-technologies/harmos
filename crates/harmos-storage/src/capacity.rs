/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use tokio::time::Instant;

use super::manager::{Inner, State};
use crate::{Error, Result};

pub(super) enum Refusal {
    Exceeded(&'static str),
    Elapsed,
    Failed(Error),
}

pub(super) enum Attempt<T> {
    Ready(T),
    Refuse(Refusal),
    Wait,
}

impl Refusal {
    pub(super) fn into_error(self) -> Error {
        match self {
            Self::Exceeded(resource) => Error::ResourceExhausted { resource },
            Self::Elapsed => Error::ResourceExhausted { resource: "storage admission deadline" },
            Self::Failed(error) => error,
        }
    }
}

impl Inner {
    #[tracing::instrument(name = "storage.capacity.wait", level = "trace", skip_all)]
    pub(super) async fn wait<T>(
        &self, deadline: Option<Instant>, mut attempt: impl FnMut(&mut State) -> Attempt<T>,
    ) -> std::result::Result<T, Refusal> {
        let mut changed = self.changed.subscribe();
        loop {
            changed.borrow_and_update();
            match attempt(&mut self.state.lock().unwrap()) {
                Attempt::Ready(value) => return Ok(value),
                Attempt::Refuse(refusal) => return Err(refusal),
                Attempt::Wait => {}
            }
            let notified = match deadline {
                Some(deadline) => tokio::time::timeout_at(deadline, changed.changed()).await.map_err(|_| Refusal::Elapsed)?,
                None => changed.changed().await,
            };
            notified.map_err(|_| Refusal::Failed(Error::Closing { domain: "storage" }))?;
        }
    }
}

impl Inner {
    #[tracing::instrument(name = "storage.capacity.admit_when", level = "trace", skip_all)]
    pub(super) async fn admit_when<T>(&self, mut check: impl FnMut(&mut State) -> Result<Option<T>>) -> Result<T> {
        self.wait(None, |state| match check(state) {
            Ok(Some(value)) => Attempt::Ready(value),
            Ok(None) => Attempt::Wait,
            Err(error) => Attempt::Refuse(Refusal::Failed(error)),
        })
        .await
        .map_err(Refusal::into_error)
    }
}

pub(super) fn admitting(state: &State, write: bool) -> Result<()> {
    if state.status.closing {
        return Err(Error::Closing { domain: "storage" });
    }
    if !state.ready || !state.status.running {
        return Err(Error::NotReady { domain: "storage" });
    }
    if write && let Some(source) = &state.status.failure {
        return Err(Error::Persistence { source: source.clone() });
    }
    Ok(())
}
