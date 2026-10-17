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
impl Cleanup {
    pub(super) async fn finish(&mut self) -> Result<(), String> {
        if let Some(instance) = self.instance.as_mut() {
            instance.quiesce();
        }
        if let Some(endpoint) = &self.endpoint {
            endpoint.close();
        }
        for task in self.routed_tasks.values_mut() {
            if let Some(cancellation) = task.cancellation.take() {
                let _ = cancellation.send(());
            }
        }
        // Native Finalizers May Be Children Of The Routed Calls Below.
        // Prove Their Carrier Ended Before Joining Those Host-Side Parents.
        if let Some(session) = self.session.as_mut() {
            session.end().await?;
        }
        if let Some(ended) = self.ended.take() {
            ended.send_replace(true);
        }
        let ids: Vec<_> = self.routed_tasks.keys().copied().collect();
        for id in ids {
            let task = self.routed_tasks.get_mut(&id).expect("the retained slot exists");
            if task.failure.is_some() {
                continue;
            }
            if let Err(error) = (&mut task.running).await {
                let failure = format!("native operation drain failed: {}", harmos_domain::failure::Report(&error));
                task.failure = Some(failure.clone());
                self.failure.get_or_insert(failure);
                continue;
            }
            self.routed_tasks.remove(&id);
        }
        for reader in &self.readers {
            reader.0.abort();
        }
        while let Some(reader) = self.readers.last_mut() {
            let joined = (&mut reader.0).await;
            let reader = self.readers.pop().expect("the retained reader exists");
            if let Err(error) = joined
                && !error.is_cancelled()
            {
                self.failure.get_or_insert_with(|| format!("native transport task failed: {error}"));
                self.failed_readers.push(reader);
            }
        }
        if let Some(session) = self.session.as_mut() {
            session.finish().await?;
            if let Some(failure) = session.outcome() {
                self.logical_failure.get_or_insert_with(|| failure.to_string());
            }
        }
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        self.child_scopes.clear();
        self.session.take();
        self.endpoint.take();
        self.admitted.clear();
        self.registration.take();
        self.instance.take();
        Ok(())
    }
}

impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl Calls {
    pub(super) fn new(registrations: tokio::sync::watch::Sender<Option<Vec<Registration>>>) -> Self {
        Self {
            live: Mutex::new(None),
            ready: tokio::sync::watch::channel(false).0,
            staged: tokio::sync::watch::channel(false).0,
            registrations,
        }
    }

    pub(super) fn open(
        &self, live: mpsc::UnboundedSender<Request>, arrivals: mpsc::Sender<Arrival>, operations: &[crate::operations::Definition],
        contracts: &[crate::operations::Contract], ended: tokio::sync::watch::Receiver<bool>,
        plugin: Option<(crate::plugin::PluginScope, Arc<observers::Remote>)>,
    ) -> Arc<Endpoint> {
        let scope = plugin.as_ref().map(|(scope, _)| scope.clone());
        let endpoint = Endpoint::new(live, arrivals, operations, contracts, ended, scope);
        let endpoint = match &plugin {
            Some((_, remote)) => endpoint.with_observers(remote),
            None => endpoint,
        };
        let endpoint = Arc::new(endpoint);
        let mut current = self.live.lock().expect("the sidecar call lock is healthy");
        let previous = current.replace(endpoint.clone());
        if let Some(previous) = previous.as_ref() {
            previous.close();
        }
        drop(current);
        drop(previous);
        self.staged.send_replace(true);
        self.ready.send_replace(*endpoint.initialized().borrow());
        endpoint
    }

    pub(super) fn close(&self) {
        let mut current = self.live.lock().expect("the sidecar call lock is healthy");
        let previous = current.take();
        if let Some(previous) = previous.as_ref() {
            previous.close();
        }
        drop(current);
        drop(previous);
        self.ready.send_replace(false);
        self.staged.send_replace(false);
        self.registrations.send_replace(None);
    }

    pub(super) fn current(&self) -> Result<Arc<Endpoint>, SidecarError> {
        let current = self.live.lock().expect("the sidecar call lock is healthy");
        current.as_ref().cloned().ok_or(SidecarError::Unavailable)
    }
}

impl Drop for RoutedTask {
    fn drop(&mut self) {
        if let Some(cancellation) = self.cancellation.take() {
            let _ = cancellation.send(());
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.0.abort();
    }
}
