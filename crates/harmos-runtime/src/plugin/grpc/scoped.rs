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
use tokio::sync::{mpsc, oneshot, watch};

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
    samples: mpsc::Sender<Samples>,
    close: watch::Sender<bool>,
    completed: watch::Receiver<Option<Outcome>>,
}

/* === Enums === */

/// A Request to Open or Settle the Scope's Sample Demand.
enum Samples {
    Start(oneshot::Sender<Result<(), Error>>),
    Stop(oneshot::Sender<Result<(), Error>>),
}

/* === Traits === */

/* === Implementations === */

impl<A: Application> Runtime<A> {
    /// Bind Calls and Descendant Time Calls Without Opening Another Connection; Samples Start Separately.
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
        let (samples, requests) = mpsc::channel(1);
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
            bindings.tasks.spawn(binding.run(requests, closing, completed));
        }
        Ok(ScopedBinding {
            operations,
            samples,
            close,
            completed: completion,
        })
    }
}

impl<A: Application> Binding<A> {
    /// Serve Calls, Open and Settle Sample Demand on Request, Then Settle at Close; the Attachment Owns This Task.
    async fn run(self, mut requests: mpsc::Receiver<Samples>, closing: watch::Receiver<bool>, completed: watch::Sender<Option<Outcome>>) {
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
        let mut publications = Publications::idle(source, &activation, &clock, &host, &operations);
        let mut sampling = false;
        loop {
            // A Dropped Handle Closes Like an Explicit Close.
            let request = tokio::select! {
                biased;
                () = super::disconnected(closing.clone()) => break,
                () = super::disconnected(parent.clone()) => break,
                request = requests.recv() => match request {
                    Some(request) => request,
                    None => break,
                },
            };
            match request {
                Samples::Start(reply) if sampling => {
                    let _ = reply.send(Err(Error::Invalid("Start Scoped Samples: demand is already open; stop it first".into())));
                }
                Samples::Start(reply) => {
                    let _activity = host.activity("Start Scoped Samples");
                    let prepare = Publications::prepare(&streams, &activation, &source.settings, &operations, true);
                    let prepared = tokio::select! {
                        biased;
                        () = super::disconnected(parent.clone()) => Err(Error::Invalid("Start Scoped Samples: activation is closing; bind to its next activation".into())),
                        () = super::disconnected(closing.clone()) => Err(Error::Invalid("Start Scoped Samples: the binding is closing".into())),
                        prepared = host.timeout(source.settings.connect_timeout, prepare) => prepared.map_err(|_| Error::Timeout("Start Scoped Samples")).and_then(|prepared| prepared),
                    };
                    let started = prepared.map(|prepared| {
                        publications = Publications::scoped(prepared, source, &activation, &clock, &host, &operations);
                        sampling = true;
                    });
                    let _ = reply.send(started);
                }
                Samples::Stop(reply) => {
                    // Calls Stay Open: Only the Demand Settles, Within `stop_grace` on the Runtime Clock.
                    let activity = host.activity("Stop Scoped Samples");
                    let deadline = host.now() + source.settings.stop_grace;
                    let stopped = activity.wait(publications.settle(&host, deadline, None, "Stop Scoped Samples")).await;
                    publications = Publications::idle(source, &activation, &clock, &host, &operations);
                    sampling = false;
                    let _ = reply.send(stopped);
                }
            }
        }
        // Settling Parks Its Runtime-Clock Activity While It Waits on the Plugin.
        let activity = host.activity("Close Scoped Plugin");
        let deadline = host.now() + source.settings.stop_grace;
        let result = activity.wait(publications.settle(&host, deadline, Some(&unit), "Close Scoped Plugin")).await;
        completed.send_replace(Some(result.map_err(Arc::new)));
    }
}

impl ScopedBinding {
    pub fn operations(&self) -> &Manager {
        &self.operations
    }

    /// Open the Scope's Samples Demand; the Host Grants Ticks Only Until `stop_samples` or `close`.
    pub async fn start_samples(&self) -> Result<(), Error> {
        self.request(Samples::Start, "Start Scoped Samples").await
    }

    /// Settle Only the Scope's Samples Demand Within `stop_grace`; Calls Stay Open.
    pub async fn stop_samples(&self) -> Result<(), Error> {
        self.request(Samples::Stop, "Stop Scoped Samples").await
    }

    async fn request(&self, request: fn(oneshot::Sender<Result<(), Error>>) -> Samples, action: &'static str) -> Result<(), Error> {
        let (reply, answer) = oneshot::channel();
        let closed = || Error::Invalid(format!("{action}: the binding has closed; bind the scope again"));
        self.samples.send(request(reply)).await.map_err(|_| closed())?;
        answer.await.map_err(|_| closed())?
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
