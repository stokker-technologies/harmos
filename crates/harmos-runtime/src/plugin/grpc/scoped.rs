/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Scoped Activation Publication Lifetime

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports
use tokio::sync::{oneshot, watch};

// Local Imports
use super::{Activation, Error, Outcome, publish::Publications};
use crate::{
    Application, Runtime,
    clock::Clock,
    operations::{Manager, clock::Unit},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// One Scope's Settlement State, Owned by the Attachment From Bind to Settled.
struct Binding<A: Application> {
    streams: crate::streams::StreamManager<A>,
    activation: Activation,
    host: Clock,
    clock: Clock,
    operations: Manager,
    unit: Arc<Unit>,
}

/// Owns One Trusted Scope's Clock, Calls and Samples on an Admitted Activation.
#[must_use = "retain the binding until its scoped work has settled"]
pub struct ScopedBinding {
    operations: Manager,
    close: watch::Sender<bool>,
    completed: watch::Receiver<Option<Outcome>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<A: Application> Runtime<A> {
    /// Bind Samples and Descendant Time Calls Without Opening Another Connection.
    pub async fn bind_grpc(&self, activation: &Activation, scope: impl Into<String>, clock: Clock) -> Result<ScopedBinding, Error> {
        let scope = scope.into();
        let source =
            activation.publications.get().ok_or_else(|| Error::Invalid("Bind Scoped Plugin: activation is not admitted".into()))?;
        if !Arc::ptr_eq(&self.operations.registry, &source.operations.registry) {
            return Err(Error::Invalid("Bind Scoped Plugin: activation belongs to another runtime".into()));
        }
        let operations =
            source.operations.with_scope(scope.clone()).map_err(|error| Error::Invalid(format!("Bind Scoped Plugin: {error}")))?;
        let unit = Arc::new(Unit::new(clock.clone()));
        let mut context = operations.call_context().clone();
        context.insert(unit.clone()).map_err(|error| Error::Invalid(format!("Bind Scoped Plugin: {error}")))?;
        let operations = operations.with_context(context);
        let (close, closing) = watch::channel(false);
        let (completed, completion) = watch::channel(None);
        let (ready, started) = oneshot::channel();
        let binding = Binding {
            streams: self.streams.clone(),
            activation: activation.clone(),
            host: self.operations.clock().clone(),
            clock,
            operations: operations.clone(),
            unit,
        };
        {
            // Admission, Registration and Ownership Change Together Under the Attachment's Lock.
            let mut bindings = source.bindings.lock().expect("scoped bindings lock is healthy");
            if bindings.closed || *source.close.borrow() || !activation.is_connected() {
                return Err(Error::Invalid("Bind Scoped Plugin: activation is closing; bind to its next activation".into()));
            }
            while bindings.tasks.try_join_next().is_some() {}
            bindings.scopes.retain(|_, completed| completed.borrow().is_none() && completed.has_changed().is_ok());
            if bindings.scopes.contains_key(&scope) {
                return Err(Error::Invalid("Bind Scoped Plugin: scope already has an active binding".into()));
            }
            bindings.scopes.insert(scope, completion.clone());
            bindings.tasks.spawn(binding.run(ready, closing, completed));
        }
        match started.await {
            Ok(Ok(())) => Ok(ScopedBinding { operations, close, completed: completion }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(Error::Invalid("Bind Scoped Plugin: activation stopped while binding; bind to its next activation".into())),
        }
    }
}

impl<A: Application> Binding<A> {
    /// Prepare, Serve Until Close, Then Settle; the Attachment Owns This Task Throughout.
    async fn run(
        self, ready: oneshot::Sender<Result<(), Error>>, closing: watch::Receiver<bool>, completed: watch::Sender<Option<Outcome>>,
    ) {
        let Self {
            streams,
            activation,
            host,
            clock,
            operations,
            unit,
        } = self;
        let source = activation.publications.get().expect("bound activations are admitted");
        let parent = source.close.clone();
        let prepared = {
            let _activity = host.activity("Bind Scoped Plugin");
            let prepare = Publications::prepare(&streams, &activation, &source.settings, &operations, true);
            tokio::select! {
                biased;
                () = super::disconnected(parent.clone()) => Err(Error::Invalid("Bind Scoped Plugin: activation is closing; bind to its next activation".into())),
                prepared = host.timeout(source.settings.connect_timeout, prepare) => prepared.map_err(|_| Error::Timeout("Bind Scoped Plugin")).and_then(|prepared| prepared),
            }
        };
        let mut publications = match prepared {
            Ok(prepared) => Publications::scoped(prepared, source, &activation, &clock, &operations),
            Err(error) => {
                // An Unprepared Binding Never Served, So It Has Nothing to Settle.
                completed.send_replace(Some(Ok(())));
                let _ = ready.send(Err(error));
                return;
            }
        };
        let _ = ready.send(Ok(()));
        // A Dropped Handle Closes Like an Explicit Close.
        tokio::select! {
            () = super::disconnected(closing) => {},
            () = super::disconnected(parent) => {},
        }
        let deadline = host.now() + source.settings.stop_grace;
        let result = publications.settle(&host, deadline, Some(&unit), "Close Scoped Plugin").await;
        completed.send_replace(Some(result.map_err(Arc::new)));
    }
}

impl ScopedBinding {
    pub fn operations(&self) -> &Manager {
        &self.operations
    }

    /// Stop Scoped Calls and Await Accepted Ingestion and Producer Cleanup Within `stop_grace`.
    pub async fn close(mut self) -> Outcome {
        self.close.send_replace(true);
        loop {
            if let Some(result) = self.completed.borrow_and_update().clone() {
                return result;
            }
            if self.completed.changed().await.is_err() {
                return Err(Arc::new(Error::Deadline { action: "Close Scoped Plugin" }));
            }
        }
    }
}

impl Drop for ScopedBinding {
    fn drop(&mut self) {
        self.close.send_replace(true);
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
