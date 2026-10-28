/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod carrier;
mod catalog;
mod connection;
mod host;
mod identity;
mod instance;
mod local;
mod observer;
mod observer_host;
mod plugin;
mod publisher;
mod task;

pub mod __private {
    pub use super::observer::{Declarations as ObserverDeclarations, Inventory as ObserverInventory, Outputs as ObserverOutputs};
    pub use super::task::spawn as spawn_observer;
    pub use crate as sidecar;
    pub use harmos_domain::{qualified, qualified_len, qualify};
    pub use harmos_sdk::__private::*;
}

/* === Re-Exports === */

pub use harmos_sdk::{ExecutionContext, Operation, Service};

pub use catalog::{CATALOG_EXPORT_FLAG, CATALOG_EXPORT_LIMIT};
pub use host::{CallError, PublicationError, Supervisor};
pub use identity::ActivationId;
pub use instance::OperationInstance;
pub use observer::{Activation, Activity, DemandKey, Instance, Observer, ObserverAttachment, ObserverContext, ObserverError, Tick};
pub use observer_host::{ObserverCaller, ObserverHost, ObserverPublisher, ObserverStreamPublisher};
pub use plugin::{PluginNative, run_plugin, serve_plugin_channels};
pub use publisher::{Batch, PublishError, Publisher};

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::sync::{mpsc, oneshot};
// Local Imports
use crate::Frame;
use carrier::{Failure, Input, Sender};
use harmos_domain::{Refusal, UNKNOWN, UNREADABLE, reserved};
use host::Outgoing;

/* === Constants === */

const INCOMING_REQUESTS: usize = 128;

/* === Macros === */

/* === Type Aliases === */

type Prepare = Box<dyn Fn(&str) -> Result<Serving, Refusal> + Send + Sync>;

type Cancellations = Arc<Mutex<BTreeMap<u64, Option<oneshot::Sender<()>>>>>;
type Table = Arc<Mutex<harmos_sdk::__private::WireTable>>;
type SharedOpening = Arc<tokio::sync::Mutex<Opening>>;
type Retirement = Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), Refusal>> + Send>> + Send + Sync>;

/* === Structs === */

struct Incoming {
    frame: Frame,
    cancelled: Option<oneshot::Receiver<()>>,
    consumption: Option<crate::transport::Consumption>,
}

struct ConnectionGuard(Supervisor);

struct Worker(tokio::task::JoinHandle<Result<(), Refusal>>);

pub(crate) struct Opening {
    serving: Option<Serving>,
    prepare: Prepare,
    catalog: crate::PluginCatalog,
}

struct Serving {
    owner: String,
    operations: Table,
    retirement: Retirement,
    quiescence: Retirement,
}

/* === Enums === */

enum Answered {
    Nothing,
    Frame(Frame),
    Deferred(Pin<Box<dyn Future<Output = Frame> + Send>>),
}

/* === Traits === */

/* === Implementations === */

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.disconnected();
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl Opening {
    async fn cleanup(&mut self) -> Result<(), Refusal> {
        let Some(serving) = &self.serving else {
            return Ok(());
        };
        let settlement = serving.operations.lock().map_err(Refusal::of)?.close();
        settlement.await.map_err(Refusal::of)?;
        (serving.retirement)().await?;
        let retired = serving.operations.lock().map_err(Refusal::of)?.release().map_err(Refusal::of)?;
        drop(retired);
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn exchange(
    opening: SharedOpening, inbound: &mut Input, outgoing: &Sender, completed: oneshot::Sender<()>,
    mut stopping: tokio::sync::watch::Receiver<bool>, clock: fn() -> Option<i64>,
) -> Result<(), Refusal> {
    let (to_host, mut outbound) = mpsc::channel(1024);
    let host = Supervisor::new(to_host.clone(), clock);
    let _connection = ConnectionGuard(host.clone());
    let (requests_tx, requests_rx) = mpsc::channel(INCOMING_REQUESTS);
    let cancellations: Cancellations = Arc::new(Mutex::new(BTreeMap::new()));
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let mut worker =
        Worker(tokio::spawn(dispatch_requests(opening, host.clone(), requests_rx, cancellations.clone(), to_host.clone(), shutdown_rx)));
    let mut abandoned = false;
    let mut active_requests = std::collections::BTreeSet::new();
    let mut exit_after_request = None;
    let mut abandon_after_request = false;
    let mut worker_finished = false;
    let mut outcome = Ok(());

    let serving = async {
        loop {
            let sending = {
                let worker_done = async {
                    if worker_finished {
                        std::future::pending::<Result<Result<(), Refusal>, tokio::task::JoinError>>().await
                    } else {
                        (&mut worker.0).await
                    }
                };
                tokio::select! {
                    arrival = inbound.receive() => match arrival {
                        Ok(Some(carrier::Arrival { frame, cancelled: early_cancelled, consumption })) => match frame {
                            Frame::Answer { id, result } => {
                                host.receive_answer(id, result);
                                if let Some(receipt) = consumption { let _ = receipt.consumed(); }
                                continue;
                            }
                            Frame::Refusal { id, refusal } => {
                                host.receive_refusal(id, refusal);
                                if let Some(receipt) = consumption { let _ = receipt.consumed(); }
                                continue;
                            }
                            Frame::Cancel { id } => {
                                if let Some(cancel) = cancellations.lock().expect("request cancellation lock")
                                    .get_mut(&id).and_then(Option::take)
                                {
                                    let _ = cancel.send(());
                                }
                                continue;
                            }
                            frame @ (Frame::Call { .. } | Frame::Push { .. }) => {
                                let cancelled = if let Frame::Call { id, .. } = &frame {
                                    let mut pending = cancellations.lock().expect("request cancellation lock");
                                    if pending.contains_key(id) {
                                        outgoing.fail(Failure::Invalid("duplicate live call id".to_owned()));
                                        break;
                                    }
                                    let (cancel, cancelled) = oneshot::channel();
                                    if early_cancelled {
                                        let _ = cancel.send(());
                                        pending.insert(*id, None);
                                    } else {
                                        pending.insert(*id, Some(cancel));
                                    }
                                    Some(cancelled)
                                } else {
                                    None
                                };
                                match requests_tx.try_send(Incoming { frame, cancelled, consumption }) {
                                    Ok(()) => continue,
                                    Err(mpsc::error::TrySendError::Full(request)) => {
                                        let id = match &request.frame { Frame::Call { id, .. } => Some(*id), _ => None };
                                        drop(request.frame);
                                        if let Some(receipt) = request.consumption { let _ = receipt.consumed(); }
                                        if let Some(id) = id {
                                            cancellations.lock().expect("request cancellation lock").remove(&id);
                                            let refusal = Refusal::of("the sidecar request queue is full");
                                            if to_host.try_send(Outgoing::Frame(Frame::Refusal {
                                                id,
                                                refusal,
                                            })).is_err() {
                                                break;
                                            }
                                            continue;
                                        }
                                        outgoing.fail(Failure::Capacity);
                                        break;
                                    }
                                    Err(mpsc::error::TrySendError::Closed(_)) => break,
                                }
                            }
                        },
                        Ok(None) => break,
                        Err(error) => { outgoing.fail(Failure::Invalid(error.to_string())); break; },
                    },
                    asked = outbound.recv() => match asked {
                        None => break,
                        Some(asked) => asked,
                    },
                    result = worker_done => {
                        worker_finished = true;
                        match result {
                            Err(error) => {
                                outcome = Err(Refusal::of(error));
                                outgoing.fail(Failure::Panicked);
                                break;
                            }
                            Ok(result) => {
                                outcome = result;
                                match outbound.try_recv() {
                                    Ok(asked) => asked,
                                    Err(_) => break,
                                }
                            },
                        }
                    }
                }
            };

            match sending {
                Outgoing::Frame(frame) => {
                    let answered = match &frame {
                        Frame::Answer { id, .. } | Frame::Refusal { id, .. } => Some(*id),
                        _ => None,
                    };
                    if send(outgoing, frame).await.is_err() {
                        break;
                    }
                    if answered.is_some_and(|id| active_requests.remove(&id)) && active_requests.is_empty() {
                        if abandon_after_request {
                            abandoned = true;
                            break;
                        }
                        if let Some(reason) = exit_after_request.take() {
                            let _ = send(outgoing, Frame::exiting(reason)).await;
                            break;
                        }
                    }
                }
                Outgoing::Started(id) => {
                    active_requests.insert(id);
                }
                Outgoing::Finished(id) => {
                    if active_requests.remove(&id) && active_requests.is_empty() {
                        if abandon_after_request {
                            abandoned = true;
                            break;
                        }
                        if let Some(reason) = exit_after_request.take() {
                            let _ = send(outgoing, Frame::exiting(reason)).await;
                            break;
                        }
                    }
                }
                Outgoing::Exiting(reason) => {
                    if !active_requests.is_empty() {
                        exit_after_request.get_or_insert(reason);
                    } else {
                        let _ = send(outgoing, Frame::exiting(reason)).await;
                        break;
                    }
                }
                Outgoing::Abandon => {
                    if !active_requests.is_empty() {
                        abandon_after_request = true;
                    } else {
                        abandoned = true;
                        break;
                    }
                }
            }
        }
    };
    tokio::select! {
        biased;
        _ = stopping.wait_for(|closing| *closing) => {}
        _ = serving => {}
    }

    host.disconnected();
    outbound.close();
    drop(requests_tx);
    let _ = shutdown_tx.send(abandoned);
    if !worker_finished {
        outcome = (&mut worker.0).await.map_err(Refusal::of).and_then(|result| result);
    }
    let _ = completed.send(());
    outcome
}

async fn dispatch_requests(
    opening: SharedOpening, host: Supervisor, mut requests: mpsc::Receiver<Incoming>, cancellations: Cancellations,
    outgoing: mpsc::Sender<Outgoing>, mut shutdown: oneshot::Receiver<bool>,
) -> Result<(), Refusal> {
    let mut abandoned = false;
    let mut replies = tokio::task::JoinSet::new();
    loop {
        let request = tokio::select! {
            biased;
            ended = &mut shutdown => {
                abandoned = ended.unwrap_or(false);
                break;
            }
            completed = replies.join_next(), if !replies.is_empty() => {
                if let Some(Err(error)) = completed {
                    tracing::warn!(%error, "carrier reply task failed");
                    break;
                }
                continue;
            }
            request = requests.recv() => request,
        };
        let Some(Incoming {
            frame: request,
            mut cancelled,
            consumption,
        }) = request
        else {
            break;
        };
        let id = match &request {
            Frame::Call { id, .. } => Some(*id),
            _ => None,
        };

        if let Frame::Call { id, .. } = &request
            && outgoing.send(Outgoing::Started(*id)).await.is_err()
        {
            break;
        }

        if replies.len() >= INCOMING_REQUESTS {
            drop(request);
            if let Some(receipt) = consumption {
                let _ = receipt.consumed();
            }
            if let Some(id) = id {
                cancellations.lock().expect("request cancellation lock").remove(&id);
                let _ = outgoing
                    .send(Outgoing::Frame(Frame::Refusal {
                        id,
                        refusal: Refusal::new(UNKNOWN, "carrier reply capacity exhausted".to_owned()),
                    }))
                    .await;
            }
            continue;
        }

        let mut was_cancelled = false;
        let answered = tokio::select! {
            biased;
            ended = &mut shutdown => {
                abandoned = ended.unwrap_or(false);
                None
            }
            _ = async {
                match cancelled.as_mut() {
                    Some(cancelled) => { let _ = cancelled.await; }
                    None => std::future::pending::<()>().await,
                }
            } => {
                was_cancelled = true;
                None
            }
            answered = async {
                let mut opening = opening.lock().await;
                answer(&mut opening, &host, request).await
            } => Some(answered),
        };
        if let Some(receipt) = consumption {
            let _ = receipt.consumed();
        }
        if let Some(id) = id {
            if !matches!(&answered, Some(Answered::Deferred(_))) {
                cancellations.lock().expect("request cancellation lock").remove(&id);
            }
            if was_cancelled {
                if outgoing.send(Outgoing::Finished(id)).await.is_err() {
                    break;
                }
                continue;
            }
        }
        let Some(answered) = answered else {
            break;
        };

        match answered {
            Answered::Deferred(reply) => {
                let outgoing = outgoing.clone();
                let cancellations = Arc::clone(&cancellations);
                replies.spawn(async move {
                    let response = tokio::select! {
                        biased;
                        _ = async {
                            match cancelled.as_mut() {
                                Some(cancelled) => { let _ = cancelled.await; }
                                None => std::future::pending::<()>().await,
                            }
                        } => id.map(Outgoing::Finished),
                        frame = reply => Some(Outgoing::Frame(frame)),
                    };
                    if let Some(response) = response {
                        let _ = outgoing.send(response).await;
                    }
                    if let Some(id) = id {
                        cancellations.lock().expect("request cancellation lock").remove(&id);
                    }
                });
            }
            Answered::Nothing => continue,
            Answered::Frame(frame) => {
                if outgoing.send(Outgoing::Frame(frame)).await.is_err() {
                    break;
                }
            }
        }
    }

    replies.abort_all();
    while replies.join_next().await.is_some() {}
    cancellations.lock().expect("request cancellation lock").clear();

    // Abandon represents process death. Every other ending joins work and
    // cleans up the state, including a handler cancelled when its connection
    // disappears; outbound host calls have already been failed by `exchange`.
    let mut opening = opening.lock().await;
    if abandoned {
        // A local double simulates process death but its native threads still
        // belong to this process. Join them before declaring the peer ended.
        if let Some(serving) = &opening.serving {
            (serving.quiescence)().await?;
        }
    } else {
        opening.cleanup().await?;
    }
    Ok(())
}

async fn answer(opening: &mut Opening, host: &Supervisor, frame: Frame) -> Answered {
    let (id, method, params) = match frame {
        Frame::Call { id, method, params } => (id, method, params),
        _ => return Answered::Nothing,
    };
    if method == Frame::REGISTER {
        let owner = match <String as crate::Message>::decode(&params) {
            Ok(owner) => owner,
            Err(error) => {
                return Answered::Frame(Frame::Refusal {
                    id,
                    refusal: Refusal::new(UNREADABLE, format!("invalid owner assignment: {error}")),
                });
            }
        };
        if let Some(serving) = &opening.serving {
            if serving.owner != owner {
                return Answered::Frame(Frame::Refusal {
                    id,
                    refusal: Refusal::of("carrier owner assignment cannot change"),
                });
            }
        } else {
            match (opening.prepare)(&owner) {
                Ok(serving) => opening.serving = Some(serving),
                Err(error) => return Answered::Frame(Frame::Refusal { id, refusal: error }),
            }
        }
        return match serde_json::to_vec(&opening.catalog) {
            Ok(result) => Answered::Frame(Frame::Answer { id, result: result.into() }),
            Err(error) => Answered::Frame(Frame::Refusal {
                id,
                refusal: Refusal::new(UNREADABLE, error.to_string()),
            }),
        };
    }
    if method == crate::operations::WIRE_METHOD {
        let command = match crate::operations::WireCommand::decode(&params) {
            Ok(command) => command,
            Err(error) => {
                return Answered::Frame(Frame::Refusal {
                    id,
                    refusal: Refusal::new(UNREADABLE, error.to_string()),
                });
            }
        };
        let Some(serving) = &opening.serving else {
            return Answered::Frame(Frame::Refusal {
                id,
                refusal: Refusal::of("carrier owner has not been assigned"),
            });
        };
        let table = &serving.operations;
        let operation_host = match &command {
            crate::operations::WireCommand::Initialize { context, .. } => Ok(host.for_operation(*context)),
            crate::operations::WireCommand::Attempt { context, values, .. }
            | crate::operations::WireCommand::StreamAttempt { context, values, .. } => host.for_attempt(*context, values.clone()),
            _ => Ok(host.clone()),
        };
        let operation_host = match operation_host {
            Ok(operation_host) => operation_host,
            Err(error) => {
                return Answered::Frame(Frame::Refusal {
                    id,
                    refusal: Refusal::new(UNREADABLE, error.to_string()),
                });
            }
        };
        let phase = match table.lock() {
            Ok(mut table) => {
                let call = operation_host.context().clone();
                let mut dependencies = harmos_sdk::__private::ResourceSet::new();
                let caller = Arc::new(harmos_sdk::operations::Calls(Arc::new(operation_host.clone())));
                if let Err(error) = dependencies.insert(caller).and_then(|()| dependencies.insert(Arc::new(operation_host))) {
                    return Answered::Frame(Frame::Refusal { id, refusal: Refusal::of(error) });
                }
                let context = crate::ExecutionContext::new(dependencies, call);
                table.begin(command, Arc::new(context))
            }
            Err(error) => {
                return Answered::Frame(Frame::Refusal {
                    id,
                    refusal: Refusal::new(UNREADABLE, error.to_string()),
                });
            }
        };
        return Answered::Deferred(Box::pin(async move {
            let answer = phase.await;
            // Refuse Before Serialization
            // Binary Envelopes Add Framing Bytes To The Unchanged Payload.
            let minimum = match &answer {
                crate::operations::WireAnswer::Ready { payload } => payload.len(),
                crate::operations::WireAnswer::Stream { payload, .. } => payload.as_ref().map_or(0, Vec::len),
                crate::operations::WireAnswer::Failed { failure } => failure.cause().len(),
            };
            if minimum >= Frame::LIMIT {
                return oversized_reply(id);
            }
            match answer.encode() {
                Ok(result) => Frame::Answer { id, result: result.into() },
                Err(harmos_sdk::operations::WireCodecError::Oversized) => oversized_reply(id),
                Err(error) => Frame::Refusal {
                    id,
                    refusal: Refusal::new(UNREADABLE, error.to_string()),
                },
            }
        }));
    }
    if reserved(&method) {
        return Answered::Frame(Frame::Refusal {
            id,
            refusal: Refusal::new(UNKNOWN, format!("this sidecar does not serve the reserved `{method}`")),
        });
    }

    Answered::Frame(Frame::Refusal {
        id,
        refusal: Refusal::new(
            UNKNOWN,
            format!("operation work must use the `{}` phase contract; received `{method}`", crate::operations::WIRE_METHOD),
        ),
    })
}

fn oversized_reply(id: u64) -> Frame {
    Frame::Refusal {
        id,
        refusal: Refusal::new(Frame::OVERSIZED, format!("the route reply exceeds the {}-byte sidecar frame limit", Frame::LIMIT)),
    }
}

async fn send(outgoing: &Sender, frame: Frame) -> std::io::Result<()> {
    let call = match &frame {
        Frame::Answer { id, .. } | Frame::Refusal { id, .. } => Some(*id),
        _ => None,
    };
    let frame = match call {
        Some(id) if !frame.fits() => oversized_reply(id),
        _ => frame,
    };
    outgoing.send(frame).await
}

/* === Tests === */
