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

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    pub(crate) async fn admit(
        &self, domain: Domain, bytes: usize, count: usize, build: impl FnOnce(u64) -> Result<Operation>,
    ) -> Result<u64> {
        self.admit_mutation(domain, bytes, count, None, false, build).await
    }

    pub(crate) async fn admit_mutation(
        &self, domain: Domain, bytes: usize, count: usize, mutation: Option<Mutation<'_>>, measurement: bool,
        build: impl FnOnce(u64) -> Result<Operation>,
    ) -> Result<u64> {
        if mutation.is_some() && !self.inner.primary.supports_mutation_receipts() {
            return Err(Error::InvalidInput { field: "storage mutation", reason: "backend does not support mutation receipts" });
        }
        // FIFO Within Each Class; Alternate Classes When Both Are Waiting.
        let _class = if measurement { self.inner.measurement_admission.lock().await } else { self.inner.control_admission.lock().await };
        let counter = if measurement { &self.inner.measurement_waiting } else { &self.inner.control_waiting };
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let _waiting = WriteWaiter { inner: self.inner.clone(), measurement };
        let bytes = bytes.max(1);
        let byte_capacity = self.inner.settings.request.max_pending_write_bytes.get();
        let batch_capacity = self.inner.settings.request.max_pending_write_batches.get();
        let admitted = self
            .inner
            .wait(None, |state| {
                if let Err(error) = crate::capacity::admitting(state, Pool::Write) {
                    return Attempt::Refuse(Refusal::Failed(error));
                }
                if let Some(mutation) = mutation {
                    match state.receipts.lookup(mutation.id, &mutation.fingerprint, None) {
                        Ok(ReceiptState::Complete(value)) => {
                            return match value.cursor() {
                                Ok(cursor) => Attempt::Ready(Admitted::Replayed(cursor.sequence)),
                                Err(error) => Attempt::Refuse(Refusal::Failed(error)),
                            };
                        }
                        Ok(ReceiptState::Pending) => {
                            return Attempt::Refuse(Refusal::Failed(Error::InvalidInput {
                                field: "storage mutation",
                                reason: "mutation already owns different work",
                            }));
                        }
                        Ok(ReceiptState::Missing) => {}
                        Err(error) => return Attempt::Refuse(Refusal::Failed(error)),
                    }
                }
                if bytes > byte_capacity {
                    return Attempt::Refuse(Refusal::Exceeded(Pool::Write));
                }
                state.collect();

                // FIFO Within Each Class; Alternate Classes When Both Are Waiting.
                let opposite = if measurement { &self.inner.control_waiting } else { &self.inner.measurement_waiting };
                let turn = state.prefer_measurement == measurement || opposite.load(std::sync::atomic::Ordering::SeqCst) == 0;
                if !(turn
                    && state.reserved_batches == 0
                    && state.finishing.is_none()
                    && state.status.pending_bytes <= byte_capacity - bytes
                    && state.status.pending_batches < batch_capacity)
                {
                    self.inner.relieve_pressure(state, bytes, 1);
                    return Attempt::Wait;
                }
                let sequence = match domain {
                    Domain::Data => state.status.accepted.sequence,
                    Domain::Recovery => state.status.recovery_accepted.sequence,
                };
                let (Some(last), Some(index)) = (sequence.checked_add(count as u64), state.next.checked_add(1)) else {
                    return Attempt::Refuse(Refusal::Failed(Error::InvalidInput {
                        field: "storage admission",
                        reason: "position overflow",
                    }));
                };
                state.reserved = bytes;
                state.reserved_batches = 1;
                state.collect();
                Attempt::Ready(Admitted::Reserved(sequence, last, index))
            })
            .await
            .map_err(Refusal::into_error)?;
        let (sequence, last, index) = match admitted {
            Admitted::Replayed(sequence) => return Ok(sequence),
            Admitted::Reserved(sequence, last, index) => (sequence, last, index),
        };
        // The original exclusive reservation owns capacity and capture order.
        // Producer code runs without the state mutex; unwind releases the permit.
        let mut admission = Admission { inner: self.inner.clone(), active: true };
        let operation = build(if count == 0 { sequence } else { sequence + 1 })?;
        operation.check_entry_bytes(self.inner.settings.max_batch_bytes())?;
        if let Operation::Snapshot(snapshot) = &operation
            && (snapshot.len() != 1 || snapshot.as_slice()[0].through.sequence > sequence)
        {
            return Err(Error::InvalidInput { field: "recovery snapshot", reason: "snapshot is ahead of admitted transitions" });
        }
        let bytes = match &operation {
            Operation::ObjectGroup { bytes: retained, .. } => *retained,
            _ => bytes,
        };
        let pending = Arc::new(Pending::new(&self.inner, None, index, bytes, operation));
        {
            let mut state = self.inner.state.lock().unwrap();
            if state.status.closing {
                return Err(Error::Closing { domain: "storage" });
            }
            if let Some(source) = &state.status.failure {
                return Err(Error::Persistence { source: source.clone() });
            }
            if let Some(mutation) = mutation {
                let outcome = MutationValue::Cursor(self.position(last));
                state.receipts.insert(mutation.id, mutation.fingerprint, crate::MemorySize::memory_size(&outcome))?;
                state.receipts.complete(mutation.id, outcome);
            }
            let target = Target { index, sequence: last };
            match domain {
                Domain::Data => {
                    state.status.accepted.sequence = last;
                    state.data_accepted = target;
                }
                Domain::Recovery => {
                    state.status.recovery_accepted.sequence = last;
                    state.recovery_accepted = target;
                }
            }
            state.next = index;
            state.prefer_measurement = !measurement;
            state.reserved = 0;
            state.reserved_batches = 0;
            state.queue.push_back(pending);
            state.collect();
            admission.active = false;
        }
        self.inner.notify();
        Ok(last)
    }

    pub(super) async fn flush_domain(&self, domain: Domain, through: Option<&StorageCursor>) -> Result<()> {
        let mut changed = self.inner.changed.subscribe();
        let target = {
            let mut state = self.inner.state.lock().unwrap();
            if !state.ready {
                return Err(Error::NotReady { domain: "storage" });
            }
            let target = if let Some(through) = through {
                state.data_target(through)?
            } else {
                match domain {
                    Domain::Data => state.data_accepted,
                    Domain::Recovery => state.recovery_accepted,
                }
            };
            // Explicit retry permits another bounded attempt after a failure.
            if state.status.failure.is_some() {
                state.status.failure = None;
                state.retry = state.retry.wrapping_add(1);
            }
            target
        };
        self.inner.notify();
        loop {
            let notify;
            {
                let mut state = self.inner.state.lock().unwrap();
                let flushed = match domain {
                    Domain::Data => state.data_flushed,
                    Domain::Recovery => state.recovery_flushed,
                };
                if flushed >= target.index {
                    return Ok(());
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence { source: source.clone() });
                }
                if !state.status.running {
                    return Err(Error::NotReady { domain: "storage worker" });
                }
                let request = match domain {
                    Domain::Data => &mut state.data_request,
                    Domain::Recovery => &mut state.recovery_request,
                };
                notify = request.is_none_or(|current| target.index < current.index);
                if notify {
                    *request = Some(target);
                }
            }
            if notify {
                self.inner.notify();
            }
            changed.changed().await.map_err(|_| Error::NotReady { domain: "storage" })?;
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
