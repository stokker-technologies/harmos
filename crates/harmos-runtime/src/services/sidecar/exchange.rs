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

impl<A, H> Exchange<'_, A, H>
where
    A: Application,
    H: Fn(&Service<A>, Notice) + Send + Sync,
{
    pub(super) async fn arrival(&mut self) -> Option<Arrival> {
        tokio::select! {
            reply = self.returned.recv() => reply.map(|(frame, token)| Arrival::Reply(frame, token)),
            arrival = self.inbound.recv() => arrival,
        }
    }

    pub(super) async fn serve(
        &mut self, requests: &mut mpsc::UnboundedReceiver<Request>, mut initialized: tokio::sync::watch::Receiver<bool>,
        on_ready: impl FnOnce(&mut Service<A>) -> Result<(), String>,
    ) -> bool {
        let plugin = self.plugin.clone().expect("canonical native execution retains its original scope");
        let activation = plugin.ready();
        tokio::pin!(activation);
        let mut committed = plugin.is_ready();
        let mut activation_pending = !committed;
        let mut on_ready = Some(on_ready);
        loop {
            if *initialized.borrow_and_update()
                && committed
                && plugin.is_ready()
                && let Some(ready) = on_ready.take()
                && let Err(error) = ready(self.context)
            {
                self.failure = Some(error);
                return false;
            }
            let step = if on_ready.is_some() {
                tokio::select! {
                    changed = initialized.changed() => {
                        if changed.is_err() { return false; }
                        continue;
                    }
                    result = &mut activation, if activation_pending => {
                        activation_pending = false;
                        // The Lifecycle Owner May Already Be Shutting Down.
                        // Its Retained Service Lease Still Owns SDK Cleanup.
                        committed = result.is_ok();
                        continue;
                    }
                    step = self.step(requests) => step,
                }
            } else {
                self.step(requests).await
            };
            match step {
                Step::Stopping => return true,
                Step::Published(Err(crate::streams::StreamPublicationError::Revoked)) if plugin.is_closed() && !plugin.has_committed() => {
                    // Aborted Staging Discards Rows Without Ending SDK Cleanup.
                }
                Step::Published(publication) => {
                    let delivered = publication.map_err(|error| error.to_string()).and_then(|publication| self.deliver(publication));
                    if let Err(error) = delivered {
                        self.failure = Some(error);
                        return false;
                    }
                }
                Step::Arrived(arrival) => match self.absorb(arrival).await {
                    Absorbed::Carried => {}
                    Absorbed::Malformed(error) => {
                        self.failure = Some(error);
                        return false;
                    }
                    Absorbed::Ended => return false,
                },
                Step::Cancelled(id) => {
                    self.child_scopes.remove(&id);
                    self.pending.remove(&id);
                    if self.send(Frame::Cancel { id }).await.is_err() {
                        return false;
                    }
                }
                Step::Requested(request) => {
                    if request.answer.is_closed() {
                        continue;
                    }
                    if request.children.is_some() && self.child_scopes.len() >= 128 {
                        let _ = request.answer.send(Err(SidecarError::refused(UNKNOWN, "native attempt context capacity exhausted")));
                        continue;
                    }
                    let id = self.mint();
                    let frame = Frame::Call { id, method: request.method, params: request.params };
                    if !frame.fits() {
                        let _ = request.answer.send(Err(SidecarError::refused(
                            Frame::OVERSIZED,
                            format!("the route request exceeds the {}-byte sidecar frame limit", Frame::LIMIT),
                        )));
                        continue;
                    }
                    match self.send(frame).await {
                        Ok(()) => {
                            if let Some(children) = request.children {
                                self.child_scopes.insert(id, children);
                            }
                            self.pending.insert(id, request.answer);
                        }
                        Err(_) => {
                            let _ = request.answer.send(Err(SidecarError::Unavailable));
                        }
                    }
                }
            }
        }
    }

    pub(super) async fn step(&mut self, requests: &mut mpsc::UnboundedReceiver<Request>) -> Step {
        let Self { inbound, returned, context, pending, deliveries, delivery_turn, manager_owned_stop, .. } = self;
        let mut stopping = std::pin::pin!(context.stopping());

        std::future::poll_fn(|cx| {
            if !*manager_owned_stop && stopping.as_mut().poll(cx).is_ready() {
                return Poll::Ready(Step::Stopping);
            }
            for (id, answer) in pending.iter_mut() {
                if answer.poll_closed(cx).is_ready() {
                    return Poll::Ready(Step::Cancelled(*id));
                }
            }
            if let Poll::Ready(Some((frame, token))) = returned.poll_recv(cx) {
                return Poll::Ready(Step::Arrived(Arrival::Reply(frame, token)));
            }
            if !*delivery_turn && let Poll::Ready(arrival) = inbound.poll_recv(cx) {
                *delivery_turn = true;
                return Poll::Ready(Step::Arrived(arrival.unwrap_or(Arrival::Ended)));
            }
            if let Some(delivery) = deliveries.front_mut()
                && let Poll::Ready(publication) = delivery.as_mut().poll(cx)
            {
                deliveries.pop_front();
                *delivery_turn = false;
                return Poll::Ready(Step::Published(publication));
            }
            if let Poll::Ready(arrival) = inbound.poll_recv(cx) {
                *delivery_turn = true;
                return Poll::Ready(Step::Arrived(arrival.unwrap_or(Arrival::Ended)));
            }
            if let Poll::Ready(Some(request)) = requests.poll_recv(cx) {
                return Poll::Ready(Step::Requested(request));
            }
            Poll::Pending
        })
        .await
    }

    pub(super) async fn absorb(&mut self, arrival: Arrival) -> Absorbed {
        let (arrival, receipt) = match arrival {
            Arrival::Binary(received) => {
                if received.cancelled && matches!(received.frame, Frame::Call { .. }) {
                    drop(received.frame);
                    if let Some(receipt) = received.consumption {
                        let _ = receipt.consumed();
                    }
                    return Absorbed::Carried;
                }
                (Arrival::Frame(received.frame), received.consumption)
            }
            arrival => (arrival, None),
        };
        let result = self.absorb_frame(arrival).await;
        if let Some(receipt) = receipt {
            let _ = receipt.consumed();
        }
        result
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
