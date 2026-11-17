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
use std::{collections::BTreeSet, sync::Arc};

// Third Party Imports
use tokio::sync::{mpsc, oneshot, watch};

// Local Imports
use super::{Activation, Error, LoadOptions, Outcome, publish::Publications};
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

#[derive(Clone)]
pub(super) struct Generation(Arc<()>);

/* === Enums === */

/// A Request to Open the Scope's Sample Demand for Its Requested Streams, or to Settle It.
enum Samples {
    Start(BTreeSet<String>, Option<LoadOptions>, oneshot::Sender<Result<Generation, Error>>),
    Stop(Option<Generation>, oneshot::Sender<Result<(), Error>>),
}

/// Cancels the Unit's Calls When the Binding Task Ends, Even When Its Attachment Aborts It.
struct StopUnit(Arc<Unit>);

/* === Traits === */

/* === Implementations === */

impl<A: Application> Runtime<A> {
    /// Bind Calls and Descendant Time Calls Without Opening Another Connection; Samples Start Separately.
    pub async fn bind_grpc(&self, activation: &Activation, scope: impl Into<String>, clock: Clock) -> Result<ScopedBinding, Error> {
        ScopedBinding::bind(self.streams.clone(), &self.operations.execution(), activation, scope.into(), clock)
    }
}

impl ScopedBinding {
    pub(super) fn bind<A: Application>(
        streams: crate::streams::StreamManager<A>, manager: &Manager, activation: &Activation, scope: String, clock: Clock,
    ) -> Result<Self, Error> {
        let source =
            activation.publications.get().ok_or_else(|| Error::Invalid("Bind Scoped Plugin: activation is not admitted".into()))?;
        if !Arc::ptr_eq(&manager.registry, &source.operations.registry) {
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
            streams,
            activation: activation.clone(),
            host: manager.clock().clone(),
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
        let _stop = StopUnit(unit.clone());
        let mut publications = Publications::idle(source, &activation, &clock, &host, &operations);
        // Holding a Share Pauses the Unscoped Windows of the Streams This Scope Samples.
        let mut sampling = None;
        // A Stop Interrupted by Close Keeps Its Deadline, So Close Still Settles Within One `stop_grace`.
        let mut deadline = None;
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
                Samples::Start(_, _, reply) if sampling.is_some() => {
                    let _ = reply.send(Err(Error::Invalid("Start Scoped Samples: demand is already open; stop it first".into())));
                }
                Samples::Start(requested, options, mut reply) => {
                    if let Some(undeclared) = requested.iter().find(|stream| !source.sampling.contains_key(*stream)) {
                        let _ = reply.send(Err(Error::Invalid(format!(
                            "Start Scoped Samples: the plugin declares no stream `{undeclared}`; request only streams it declares"
                        ))));
                        continue;
                    }
                    let _activity = host.activity("Start Scoped Samples");
                    // The Facade Already Admitted Startup; Direct Owners Enter the Load Operation Here.
                    let prepare = {
                        let (streams, activation, settings, operations) =
                            (streams.clone(), activation.clone(), source.settings.clone(), operations.clone());
                        let requested = requested.clone();
                        async move { Publications::prepare(&streams, &activation, &settings, &operations, true, Some(&requested)).await }
                    };
                    let prepared = tokio::select! {
                        biased;
                        () = super::disconnected(parent.clone()) => Err(Error::Invalid("Start Scoped Samples: activation is closing; bind to its next activation".into())),
                        () = super::disconnected(closing.clone()) => Err(Error::Invalid("Start Scoped Samples: the binding is closing; bind the scope again after close() returns".into())),
                        // A Caller That Stopped Waiting Has Withdrawn Its Start.
                        () = reply.closed() => continue,
                        prepared = async {
                            match options {
                                Some(options) => super::load::load(&source.operations, activation.owner(), options, prepare).await,
                                None => prepare.await,
                            }
                        } => prepared,
                    };
                    if reply.is_closed() {
                        continue;
                    }
                    let started = prepared.map(|prepared| {
                        let generation = Generation(Arc::new(()));
                        sampling = Some((source.sampling(&requested), generation.clone()));
                        publications = Publications::scoped(prepared, source, &activation, &clock, &host, &operations);
                        generation
                    });
                    if reply.send(started).is_err() {
                        let activity = host.activity("Withdraw Scoped Samples");
                        let stopping = host.now() + source.settings.stop_grace;
                        let stopped = activity.wait(publications.settle(&host, stopping, None, "Withdraw Scoped Samples")).await;
                        if let Err(error) = stopped {
                            completed.send_replace(Some(Err(Arc::new(error))));
                            return;
                        }
                        publications = Publications::idle(source, &activation, &clock, &host, &operations);
                        sampling = None;
                    }
                }
                Samples::Stop(generation, reply) => {
                    let same_generation = generation
                        .as_ref()
                        .is_none_or(|generation| sampling.as_ref().is_some_and(|(_, current)| Arc::ptr_eq(&generation.0, &current.0)));
                    if !same_generation {
                        let _ = reply.send(Err(Error::Invalid(
                            "Stop Scoped Samples: sample generation retired; use the current samples handle".into(),
                        )));
                        continue;
                    }
                    // Calls Stay Open: Only the Demand Settles, Within `stop_grace` on the Runtime Clock.
                    let activity = host.activity("Stop Scoped Samples");
                    let stopping = *deadline.get_or_insert(host.now() + source.settings.stop_grace);
                    let stopped = {
                        let settle = activity.wait(publications.settle(&host, stopping, None, "Stop Scoped Samples"));
                        tokio::select! {
                            biased;
                            stopped = settle => Some(stopped),
                            () = super::disconnected(closing.clone()) => None,
                            () = super::disconnected(parent.clone()) => None,
                        }
                    };
                    let Some(stopped) = stopped else {
                        let _ = reply.send(Err(Error::Invalid(
                            "Stop Scoped Samples: the binding closed while samples stopped; await close() for the outcome".into(),
                        )));
                        break;
                    };
                    deadline = None;
                    publications = Publications::idle(source, &activation, &clock, &host, &operations);
                    sampling = None;
                    let _ = reply.send(stopped);
                }
            }
        }
        // Settling Parks Its Runtime-Clock Activity While It Waits on the Plugin; Requests Are Refused at Once.
        let activity = host.activity("Close Scoped Plugin");
        let deadline = deadline.unwrap_or_else(|| host.now() + source.settings.stop_grace);
        let result = {
            let settle = activity.wait(publications.settle(&host, deadline, Some(&unit), "Close Scoped Plugin"));
            tokio::pin!(settle);
            loop {
                tokio::select! {
                    biased;
                    result = &mut settle => break result,
                    Some(request) = requests.recv() => refuse(request),
                }
            }
        };
        drop(sampling);
        requests.close();
        while let Ok(request) = requests.try_recv() {
            refuse(request);
        }
        completed.send_replace(Some(result.map_err(Arc::new)));
    }
}

impl Drop for StopUnit {
    fn drop(&mut self) {
        self.0.stop();
    }
}

impl ScopedBinding {
    pub fn operations(&self) -> &Manager {
        &self.operations
    }

    /// Open the Scope's Samples Demand for Exactly the Requested `streams`; the Host Grants Their Windows Only Until
    /// `stop_samples` or `close`, and the Activation's Other Streams Stay Under Its Unscoped Demand.
    pub async fn start_samples<S: Into<String>>(&self, streams: impl IntoIterator<Item = S>) -> Result<(), Error> {
        let streams = streams.into_iter().map(Into::into).collect::<BTreeSet<_>>();
        self.request(|reply| Samples::Start(streams, Some(LoadOptions::default()), reply), "Start Scoped Samples").await.map(|_| ())
    }

    pub(super) async fn start_generation(&self, streams: Vec<String>) -> Result<Generation, Error> {
        let streams = streams.into_iter().collect();
        self.request(|reply| Samples::Start(streams, None, reply), "Start Scoped Samples").await
    }

    pub(super) async fn stop_generation(&self, generation: Generation) -> Result<(), Error> {
        self.request(|reply| Samples::Stop(Some(generation), reply), "Stop Scoped Samples").await
    }

    /// Settle Only the Scope's Samples Demand Within `stop_grace`; Calls Stay Open.
    pub async fn stop_samples(&self) -> Result<(), Error> {
        self.request(|reply| Samples::Stop(None, reply), "Stop Scoped Samples").await
    }

    async fn request<T: Send>(
        &self, request: impl FnOnce(oneshot::Sender<Result<T, Error>>) -> Samples, action: &'static str,
    ) -> Result<T, Error> {
        let (reply, answer) = oneshot::channel();
        let closed = || Error::Invalid(format!("{action}: the binding has closed; bind the scope again"));
        self.samples.send(request(reply)).await.map_err(|_| closed())?;
        answer.await.map_err(|_| closed())?
    }

    /// Stop Scoped Calls and Await Accepted Ingestion and Producer Cleanup Within `stop_grace`.
    pub async fn close(self) -> Outcome {
        self.settle().await
    }

    pub(super) async fn settle(&self) -> Outcome {
        self.close.send_replace(true);
        let mut completed = self.completed.clone();
        loop {
            if let Some(result) = completed.borrow_and_update().clone() {
                return result;
            }
            if completed.changed().await.is_err() {
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

fn refuse(request: Samples) {
    let error = Error::Invalid("Scoped Samples: the binding is closing; bind the scope again after close() returns".into());
    match request {
        Samples::Start(_, _, reply) => {
            let _ = reply.send(Err(error));
        }
        Samples::Stop(_, reply) => {
            let _ = reply.send(Err(error));
        }
    }
}

/* === Tests === */
