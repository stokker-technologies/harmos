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
use std::fmt;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::manager::{ENTRY_OVERHEAD, MemoryReservation, ReservationKind};
use super::{QueryLimits, QueryValue, StorageManager, StorageView};
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct StorageProduction<T> {
    value: T,
    _reservation: MemoryReservation,
    execution: Option<Arc<Execution>>,
}

pub struct StorageProductionPreparation {
    allowance: NonZeroUsize,
    reservation: MemoryReservation,
    execution: Option<Arc<Execution>>,
}

pub struct StorageProductionComputation {
    pub(super) execution: Arc<Execution>,
    timeout: Duration,
}

pub(super) struct Execution {
    limits: QueryLimits,
    scratch_bytes: usize,
    state: Mutex<ExecutionState>,
    _reservation: MemoryReservation,
}

struct ExecutionState {
    remaining: Duration,
    started: Option<tokio::time::Instant>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<C> StorageManager<C> {
    pub fn prepare_production(
        &self,
        max_bytes: NonZeroUsize,
    ) -> Result<StorageProductionPreparation> {
        let reservation = self.reserve_read_now(max_bytes.get(), 1)?;
        Ok(StorageProductionPreparation {
            allowance: max_bytes,
            reservation,
            execution: None,
        })
    }

    pub fn prepare_read_production(
        &self,
        max_bytes: NonZeroUsize,
    ) -> Result<StorageProductionPreparation> {
        self.prepare_read_production_with(
            ReservationKind::Read,
            self.inner.settings.query_execution,
            max_bytes,
        )
    }

    // Native Output Prepared For A View Inherits That View's Budget.
    pub fn prepare_read_production_in(
        &self,
        view: &StorageView,
        max_bytes: NonZeroUsize,
    ) -> Result<StorageProductionPreparation> {
        if !self.same_instance(view.manager()) {
            return Err(Error::InvalidInput {
                field: "query view",
                reason: "view belongs to another manager",
            });
        }
        self.prepare_read_production_with(
            ReservationKind::View(view.id()),
            view.inherited_budget(None)?,
            max_bytes,
        )
    }

    fn prepare_read_production_with(
        &self,
        kind: ReservationKind,
        limits: QueryLimits,
        max_bytes: NonZeroUsize,
    ) -> Result<StorageProductionPreparation> {
        limits.check_ceiling(self.inner.settings.query_execution_ceiling)?;
        let metadata = std::mem::size_of::<Execution>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .ok_or_else(super::query::decoded_limit)?;
        let working = self
            .inner
            .settings
            .budget
            .read_memory_bytes
            .get()
            .checked_sub(metadata)
            .and_then(|bytes| bytes.checked_sub(2 * ENTRY_OVERHEAD))
            .ok_or_else(super::query::decoded_limit)?;
        if max_bytes.get() > working {
            return Err(super::query::decoded_limit());
        }
        // Output And Scratch Draw From The Same Shared Read Pool.
        let scratch_bytes = self
            .inner
            .settings
            .query_decoded_bytes
            .get()
            .min(working - max_bytes.get());
        let reservation = self.reserve_read_for(kind, max_bytes.get(), 1)?;
        let execution_reservation = self.reserve_read_for(kind, metadata + scratch_bytes, 1)?;
        let execution = Arc::new(Execution {
            limits,
            scratch_bytes,
            state: Mutex::new(ExecutionState {
                remaining: limits.timeout,
                started: None,
            }),
            _reservation: execution_reservation,
        });
        Ok(StorageProductionPreparation {
            allowance: max_bytes,
            reservation,
            execution: Some(execution),
        })
    }

    pub fn produce<T: MemorySize>(
        &self,
        max_bytes: NonZeroUsize,
        build: impl FnOnce(NonZeroUsize) -> Result<T>,
    ) -> Result<StorageProduction<T>> {
        let preparation = self.prepare_production(max_bytes)?;
        let value = build(preparation.allowance())?;
        preparation.finish(value)
    }
}

impl StorageProductionPreparation {
    pub fn allowance(&self) -> NonZeroUsize {
        self.allowance
    }

    pub fn execution_limits(&self) -> Option<QueryLimits> {
        self.execution.as_ref().map(|execution| execution.limits)
    }

    pub fn computation(&self) -> Result<StorageProductionComputation> {
        self.execution
            .as_ref()
            .ok_or(Error::InvalidInput {
                field: "storage production computation",
                reason: "producer has no storage-read profile",
            })?
            .begin()
    }

    pub fn finish<T: MemorySize>(self, value: T) -> Result<StorageProduction<T>> {
        let Self {
            allowance,
            mut reservation,
            execution,
        } = self;
        if let Some(execution) = &execution {
            execution.check_active()?;
        }
        let bytes = value.memory_size();
        if bytes > allowance.get() {
            return Err(Error::InvalidInput {
                field: "storage production",
                reason: "produced value exceeds its memory allowance",
            });
        }
        let charged = bytes
            .checked_add(ENTRY_OVERHEAD)
            .ok_or(Error::InvalidInput {
                field: "storage production",
                reason: "size overflow",
            })?;
        // Execution Scratch Has Its Own Reservation; Retain Only The Produced Value.
        reservation.retain_read(charged);
        Ok(StorageProduction {
            value,
            _reservation: reservation,
            execution,
        })
    }
}

impl<T> StorageProduction<T> {
    pub fn value(&self) -> &T {
        &self.value
    }
}

impl<T: MemorySize> StorageProduction<T> {
    pub fn try_into_value(self) -> Result<QueryValue<T>> {
        let Self {
            value,
            _reservation,
            execution,
        } = self;
        QueryValue::from_production(value, _reservation, execution)
    }
}

impl Execution {
    pub(super) fn reserve_output(&self, bytes: NonZeroUsize) -> Result<MemoryReservation> {
        self._reservation.projection(bytes)
    }

    pub(super) fn begin(self: &Arc<Self>) -> Result<StorageProductionComputation> {
        let mut state = self.state.lock().unwrap();
        if state.started.is_some() {
            return Err(Error::NotReady {
                domain: "storage production computation",
            });
        }
        if state.remaining.is_zero() {
            return Err(super::query::execution_timeout());
        }
        let timeout = state.remaining;
        state.started = Some(tokio::time::Instant::now());
        Ok(StorageProductionComputation {
            execution: self.clone(),
            timeout,
        })
    }

    fn check_active(&self) -> Result<()> {
        let state = self.state.lock().unwrap();
        let started = state.started.ok_or(Error::NotReady {
            domain: "storage production computation",
        })?;
        if started.elapsed() >= state.remaining {
            return Err(super::query::execution_timeout());
        }
        Ok(())
    }
}

impl StorageProductionComputation {
    pub fn scratch_bytes(&self) -> usize {
        self.execution.scratch_bytes
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn deadline(&self) -> tokio::time::Instant {
        let state = self.execution.state.lock().unwrap();
        state.started.expect("original active producer") + self.timeout
    }

    pub fn check(&self) -> Result<()> {
        self.execution.check_active()
    }
}

impl Drop for StorageProductionComputation {
    fn drop(&mut self) {
        let mut state = self.execution.state.lock().unwrap();
        let started = state.started.take().expect("original active producer");
        state.remaining = state.remaining.saturating_sub(started.elapsed());
    }
}

impl<T: fmt::Debug> fmt::Debug for StorageProduction<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(formatter)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
