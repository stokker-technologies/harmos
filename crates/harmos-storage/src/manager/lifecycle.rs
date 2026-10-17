/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use super::*;

/* === Implementations === */
impl<C> StorageManager<C> {
    // Acquire Backends Before Giving One Worker To The Host's Supervisor.
    pub async fn start_with(
        &self,
        owner: Arc<()>,
        spawn: impl FnOnce(Worker) -> Result<Box<dyn WorkerOwner>>,
    ) -> Result<()> {
        self.claim_owner(owner)?;
        let mut lifecycle = self.inner.lifecycle.write().await;
        {
            let state = self.inner.state.lock().unwrap();
            if state.status.closing { return Err(Error::Closing { domain: "storage" }); }
            if state.ready && lifecycle.startup_complete { return Ok(()); }
            if state.ready { return Err(Error::AlreadyRegistered { domain: "storage", name: WORKER_NAME.into() }); }
        }
        self.acquire_locked(&mut lifecycle).await?;
        let mut state = self.inner.state.lock().unwrap();
        if state.status.closing { return Err(Error::Closing { domain: "storage" }); }
        let worker = spawn(Worker(self.erased()))?;
        state.ready = true;
        state.status.running = true;
        drop(state);
        lifecycle.owner = Some(worker);
        lifecycle.startup_complete = true;
        self.inner.notify();
        Ok(())
    }

    pub fn settings(&self) -> &StorageSettings { &self.inner.settings }

    pub async fn acquire(&self) -> Result<()> {
        let mut lifecycle = self.inner.lifecycle.write().await;
        self.acquire_locked(&mut lifecycle).await
    }

    pub(super) async fn acquire_locked(&self, lifecycle: &mut Lifecycle) -> Result<()> {
        if self.inner.state.lock().unwrap().status.closing {
            return Err(Error::Closing { domain: "storage" });
        }
        if !lifecycle.primary_open {
            lifecycle.primary_touched = true;
            let saved = self.inner.bounded(self.inner.primary.open()).await?;
            self.inner.primary.history().check(&saved.history)?;
            let mut state = self.inner.state.lock().unwrap();
            state.status.accepted = saved.clone();
            state.status.saved = saved;
            state.data_attempted = state.status.accepted.sequence;
            state.data_accepted.sequence = state.status.accepted.sequence;
            lifecycle.primary_open = true;
        }
        if !lifecycle.files_recovered {
            self.recover_files().await?;
            lifecycle.files_recovered = true;
        }
        if !lifecycle.receipts_recovered {
            self.recover_receipts().await?;
            lifecycle.receipts_recovered = true;
        }
        if !lifecycle.recovery_open {
            lifecycle.recovery_touched = true;
            let saved = self.inner.bounded(self.inner.recovery.open()).await?;
            self.inner.recovery.history().check(&saved.history)?;
            let mut state = self.inner.state.lock().unwrap();
            state.status.recovery_accepted = saved.clone();
            state.status.recovery_saved = saved;
            state.recovery_written.sequence = state.status.recovery_accepted.sequence;
            state.recovery_accepted.sequence = state.status.recovery_accepted.sequence;
            lifecycle.recovery_open = true;
        }
        if self.inner.state.lock().unwrap().status.closing {
            return Err(Error::Closing { domain: "storage" });
        }
        Ok(())
    }

    pub fn same_instance<D>(&self, other: &StorageManager<D>) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub fn claim_owner(&self, owner: Arc<()>) -> Result<()> {
        let mut state = self.inner.state.lock().unwrap();
        if state.status.closing {
            return Err(Error::Closing { domain: "storage" });
        }
        if let Some(current) = &state.service_owner {
            if !Arc::ptr_eq(current, &owner) {
                return Err(Error::AlreadyRegistered {
                    domain: "storage owner",
                    name: WORKER_NAME.to_owned(),
                });
            }
        } else {
            state.service_owner = Some(owner);
        }
        Ok(())
    }

    pub fn cursor_accepted(&self) -> StorageCursor {
        self.inner.state.lock().unwrap().status.accepted.clone()
    }

    pub fn cursor_saved(&self) -> StorageCursor {
        self.inner.state.lock().unwrap().status.saved.clone()
    }

    pub fn status(&self) -> StorageMetrics {
        self.inner.state.lock().unwrap().status.clone()
    }


}
