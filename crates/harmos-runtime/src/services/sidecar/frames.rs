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
impl<A, H> Exchange<'_, A, H>
where
    A: Application,
    H: Fn(&Service<A>, Notice) + Send + Sync,
{
    pub(super) async fn absorb_frame(&mut self, arrival: Arrival) -> Absorbed {
        match arrival {
            Arrival::Binary(_) => unreachable!("binary ownership is consumed by absorb"),
            Arrival::Publications(receipts) => {
                self.deliveries.extend(
                    receipts
                        .into_iter()
                        .map(|receipt| Box::pin(receipt.committed()) as _),
                );
                Absorbed::Carried
            }
            Arrival::ObserverFailed(error) => Absorbed::Malformed(error),
            Arrival::Demanded { stream, wanted } => {
                self.demand.send_modify(|demand| {
                    demand.insert(stream, wanted);
                });
                Absorbed::Carried
            }
            Arrival::Frame(Frame::Answer { id, result }) => {
                self.settle(id, Ok(result));
                Absorbed::Carried
            }
            Arrival::Frame(Frame::Refusal { id, refusal }) => {
                let refused = SidecarError::Refused {
                    code: refusal.code(),
                    failure: refusal.into_failure(),
                };
                self.settle(id, Err(refused));
                Absorbed::Carried
            }
            Arrival::Frame(Frame::Push { method, params }) => {
                if method == Frame::EXITING {
                    self.announced = Some(reason(&params));
                } else {
                    return Absorbed::Malformed(
                        "application notifications require an admitted operation".to_owned(),
                    );
                }
                Absorbed::Carried
            }
            Arrival::Frame(Frame::Call { id, method, params }) => {
                if self.routed_tasks.contains_key(&id) {
                    return Absorbed::Malformed("duplicate live route call id".to_owned());
                }
                if method == crate::operations::CHILD_METHOD {
                    let submitted = executor::now();
                    let child = match crate::operations::WireChild::decode(&params) {
                        Ok(child) => child,
                        Err(error) => {
                            let refusal = harmos_domain::Refusal::of(error);
                            return self.send(Frame::Refusal { id, refusal }).await.map_or_else(
                                |error| Absorbed::Malformed(error.to_string()),
                                |()| Absorbed::Carried,
                            );
                        }
                    };
                    let scope = self
                        .child_scopes
                        .values()
                        .find(|scope| &scope.context == child.context())
                        .cloned();
                    let admission = if self.routed_tasks.len() >= CHILD_REQUESTS {
                        Err(crate::plugin::DispatchError::Busy.into_refusal())
                    } else if let Some(scope) = scope {
                        Ok(Some(scope))
                    } else {
                        Ok(None)
                    };
                    let refusal = match admission {
                        Err(refusal) => refusal,
                        Ok(scope) => {
                            let (cancellation, cancelled) = oneshot::channel();
                            let invocation = match scope {
                                Some(scope) => Box::pin(async move {
                                    match child {
                                        crate::operations::WireChild::Call(call) => {
                                            scope.call(call, submitted, cancelled).await
                                        }
                                        crate::operations::WireChild::Control(control) => {
                                            scope.control(control, cancelled).await
                                        }
                                    }
                                })
                                    as Pin<
                                        Box<
                                            dyn Future<
                                                    Output = Result<
                                                        Vec<u8>,
                                                        harmos_domain::Refusal,
                                                    >,
                                                > + Send,
                                        >,
                                    >,
                                None => {
                                    // Submitted Children Belong To An Attempt Scope
                                    let call = match child {
                                        crate::operations::WireChild::Call(call)
                                            if !call.submit =>
                                        {
                                            call
                                        }
                                        _ => {
                                            let refusal = harmos_domain::Refusal::new(
                                                harmos_domain::UNKNOWN,
                                                "submitted children require an attempt context",
                                            );
                                            return self
                                                .send(Frame::Refusal { id, refusal })
                                                .await
                                                .map_or_else(
                                                    |error| Absorbed::Malformed(error.to_string()),
                                                    |()| Absorbed::Carried,
                                                );
                                        }
                                    };
                                    match if call.generation_context.as_ref().is_some_and(|token| {
                                        self.plugin.as_ref().is_some_and(|scope| {
                                            scope.accepts_generation_context(token)
                                        })
                                    }) {
                                        self.caller
                                            .as_ref()
                                            .ok_or(crate::Error::Closing {
                                                domain: "native generation caller",
                                            })
                                            .and_then(|caller| {
                                                caller.submit_encoded_at(
                                                    &call.operation,
                                                    call.payload,
                                                    submitted,
                                                    call.values,
                                                )
                                            })
                                    } else {
                                        Err(crate::Error::Closing {
                                            domain: "native attempt or generation context",
                                        })
                                    } {
                                        Ok(invocation) => Box::pin(async move {
                                            crate::operations::children::lifecycle(
                                                invocation, cancelled,
                                            )
                                            .await
                                            .map_err(crate::operations::children::refused)
                                        }),
                                        Err(error) => {
                                            let refusal =
                                                crate::operations::children::refused(error);
                                            return self
                                                .send(Frame::Refusal { id, refusal })
                                                .await
                                                .map_or_else(
                                                    |error| Absorbed::Malformed(error.to_string()),
                                                    |()| Absorbed::Carried,
                                                );
                                        }
                                    }
                                }
                            };
                            let returns = self.returns.clone();
                            let token = Arc::new(());
                            let task_token = Arc::clone(&token);
                            let future = async move {
                                let frame = match invocation.await {
                                    Ok(result) => Frame::Answer { id, result },
                                    Err(error) => Frame::Refusal { id, refusal: error },
                                };
                                let _ = returns.send((frame, task_token)).await;
                            };
                            self.routed_tasks.insert(
                                id,
                                RoutedTask {
                                    running: executor::task(future),
                                    token,
                                    cancellation: Some(cancellation),
                                    failure: None,
                                },
                            );
                            return Absorbed::Carried;
                        }
                    };
                    return self.send(Frame::Refusal { id, refusal }).await.map_or_else(
                        |error| Absorbed::Malformed(error.to_string()),
                        |()| Absorbed::Carried,
                    );
                }
                let refusal = if self.routed_tasks.len() >= CHILD_REQUESTS {
                    crate::plugin::DispatchError::Busy.into_refusal()
                } else if let Some(caller) = &self.caller {
                    match caller.submit_encoded(&method, params) {
                        Ok(invocation) => {
                            let returns = self.returns.clone();
                            let token = Arc::new(());
                            let task_token = Arc::clone(&token);
                            let (cancellation, cancelled) = oneshot::channel();
                            let future = async move {
                                let outcome =
                                    crate::operations::children::lifecycle(invocation, cancelled)
                                        .await;
                                let frame = match outcome {
                                    Ok(result) => Frame::Answer { id, result },
                                    Err(error) => {
                                        let refusal = crate::operations::children::refused(error);
                                        Frame::Refusal { id, refusal }
                                    }
                                };
                                let _ = returns.send((frame, task_token)).await;
                            };
                            self.routed_tasks.insert(
                                id,
                                RoutedTask {
                                    running: executor::task(future),
                                    token,
                                    cancellation: Some(cancellation),
                                    failure: None,
                                },
                            );
                            return Absorbed::Carried;
                        }
                        Err(error) => crate::operations::children::refused(error),
                    }
                } else {
                    harmos_domain::Refusal::new(
                        -32_606,
                        "no managed operation caller is configured",
                    )
                };
                self.send(Frame::Refusal { id, refusal }).await.map_or_else(
                    |error| Absorbed::Malformed(error.to_string()),
                    |()| Absorbed::Carried,
                )
            }
            Arrival::Frame(Frame::Cancel { id }) => {
                if let Some(task) = self.routed_tasks.get_mut(&id)
                    && let Some(cancel) = task.cancellation.take()
                {
                    let _ = cancel.send(());
                }
                // Keep The Slot Until Its Original OperationInstance Physically Settles
                Absorbed::Carried
            }
            Arrival::Reply(frame, token) => {
                let id = match &frame {
                    Frame::Answer { id, .. } | Frame::Refusal { id, .. } => *id,
                    _ => unreachable!("routed tasks produce only replies"),
                };
                // A cancelled id can be reused before its already-queued reply
                // is consumed. Only the task that owns the current slot may reply.
                if !self
                    .routed_tasks
                    .get(&id)
                    .is_some_and(|current| Arc::ptr_eq(&current.token, &token))
                {
                    return Absorbed::Carried;
                }
                let task = self
                    .routed_tasks
                    .get_mut(&id)
                    .expect("the reply owns its slot");
                if let Err(error) = (&mut task.running).await {
                    let failure = format!(
                        "native operation drain failed: {}",
                        harmos_domain::failure::Report(&error)
                    );
                    task.failure = Some(failure.clone());
                    *self.cleanup_failure = Some(failure.clone());
                    return Absorbed::Malformed(failure);
                }
                self.routed_tasks.remove(&id);
                self.send(frame).await.map_or_else(
                    |error| Absorbed::Malformed(error.to_string()),
                    |()| Absorbed::Carried,
                )
            }
            Arrival::Diagnostic(line) => {
                (self.notice)(self.context, Notice::Logged { line });
                Absorbed::Carried
            }
            Arrival::Ended => Absorbed::Ended,
        }
    }

    pub(super) fn deliver(&mut self, publication: ManagedPublication) -> Result<(), String> {
        if let Some(ingest) = self.ingress.streams.get(publication.stream()) {
            ingest(self.context, &publication)?;
            return Ok(());
        }

        (self.notice)(self.context, Notice::Published { publication });

        Ok(())
    }

    pub(super) fn settle(&mut self, id: u64, answer: Result<Vec<u8>, SidecarError>) {
        self.child_scopes.remove(&id);
        if let Some(waiting) = self.pending.remove(&id) {
            let _ = waiting.send(answer);
        }
    }

    pub(super) fn mint(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        id
    }

    pub(super) async fn send(&mut self, frame: Frame) -> std::io::Result<()> {
        self.outgoing.send(frame).await
    }
}
