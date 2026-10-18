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
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

// Third Party Imports

// Local Imports
use super::{
    OperationGeneration, OperationInstance, OperationInstanceId, OperationManager, OperationStatus, StreamInput, StreamItem, StreamOutput,
    StreamSettings, stream,
};
use crate::operations::OperationDeclaration;
use crate::supervision::instance::{Instance, Phase};
use crate::{Error, Refusal, Result};
use harmos_domain::MemorySize;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub type Upload<O> = Session<O, SessionInput<<<O as OperationDeclaration>::Input as crate::operations::Direction>::Item>, ()>;
pub type Download<O> = Session<O, (), SessionOutput<<<O as OperationDeclaration>::Output as crate::operations::Direction>::Item>>;
pub type Exchange<O> = Session<
    O,
    SessionInput<<<O as OperationDeclaration>::Input as crate::operations::Direction>::Item>,
    SessionOutput<<<O as OperationDeclaration>::Output as crate::operations::Direction>::Item>,
>;

/* === Structs === */
#[must_use = "dropping a streaming session requests cancellation"]
pub struct Session<O: OperationDeclaration, I = (), R = ()> {
    input: I,
    output: R,
    completion: Completion<O>,
}

pub struct SessionInput<T> {
    sender: Option<StreamOutput<T>>,
    ends: Arc<Ends>,
    control: Arc<Instance>,
}

pub struct SessionOutput<T> {
    receiver: StreamInput<T>,
    ends: Arc<Ends>,
    control: Arc<Instance>,
}

#[must_use = "join completion or explicitly cancel and settle"]
pub struct Completion<O: OperationDeclaration> {
    instance: OperationInstance<O>,
    ends: Arc<Ends>,
    delivered: bool,
}

// Channel Progress Only; Execution And Cleanup Remain On Instance.
struct Ends {
    input: AtomicBool,
    output: AtomicBool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<O: OperationDeclaration, I, R> Session<O, I, R> {
    pub fn id(&self) -> OperationInstanceId {
        self.completion.id()
    }

    pub fn generation(&self) -> OperationGeneration {
        self.completion.generation()
    }

    pub fn status(&self) -> OperationStatus {
        self.completion.status()
    }

    pub fn cancel(&self) {
        self.completion.cancel();
    }

    pub fn split(self) -> (I, R, Completion<O>) {
        (self.input, self.output, self.completion)
    }

    pub async fn join(self) -> Result<crate::operations::Response<O::Response>> {
        let (input, output, completion) = self.split();
        drop(input);
        drop(output);
        completion.join().await
    }

    pub async fn cancel_and_settle(self) -> Result<()> {
        self.cancel();
        let (input, output, completion) = self.split();
        drop(input);
        drop(output);
        completion.cancel_and_settle().await
    }
}

impl<O: OperationDeclaration, T, R> Session<O, SessionInput<T>, R> {
    pub fn finish_input(&mut self) {
        self.input.finish_input();
    }
}
impl<O: OperationDeclaration, T: MemorySize, R> Session<O, SessionInput<T>, R> {
    pub async fn send(&self, item: T) -> Result<()> {
        self.input.send(item).await
    }

    pub async fn send_with(&self, bytes: usize, make: impl FnOnce() -> Result<T>) -> Result<()> {
        self.input.send_with(bytes, make).await
    }
}
impl<O: OperationDeclaration, I, T> Session<O, I, SessionOutput<T>> {
    pub async fn next(&mut self) -> Result<Option<StreamItem<T>>> {
        self.output.next().await
    }
}

impl<T> SessionInput<T> {
    pub fn finish_input(&mut self) {
        self.ends.input.store(true, Ordering::Release);
        self.sender.take();
    }
}
impl<T: MemorySize> SessionInput<T> {
    pub async fn send(&self, item: T) -> Result<()> {
        self.send_with(item.memory_size(), || Ok(item)).await
    }

    pub async fn send_with(&self, bytes: usize, make: impl FnOnce() -> Result<T>) -> Result<()> {
        let sender = self.sender.as_ref().ok_or(Error::NotReady { domain: "operation input closed" })?;
        tokio::select! {
            biased;
            error = ended(&self.control) => Err(error),
            result = sender.send_with(bytes, make) => result,
        }
    }
}
impl<T> Drop for SessionInput<T> {
    fn drop(&mut self) {
        if !self.ends.input.load(Ordering::Acquire) {
            cancel_unsettled(&self.control);
        }
    }
}

impl<T> SessionOutput<T> {
    pub async fn next(&mut self) -> Result<Option<StreamItem<T>>> {
        if self.ends.output.load(Ordering::Acquire) {
            return Ok(None);
        }
        let mut status = self.control.settlement();
        loop {
            let current = status.borrow_and_update().clone();
            if current.cancellation_requested() {
                self.receiver.close();
                return Err(Refusal::Cancelled.into());
            }
            if current.outcome().is_some() {
                self.receiver.close();
                let item = self.receiver.try_recv().ok();
                if item.is_none() {
                    self.ends.output.store(true, Ordering::Release);
                }
                return Ok(item);
            }
            tokio::select! {
                biased;
                item = self.receiver.recv() => {
                    if item.is_none() { self.ends.output.store(true, Ordering::Release); }
                    return Ok(item);
                }
                changed = status.changed() => {
                    if changed.is_err() { return Err(Refusal::Unavailable.into()); }
                }
            }
        }
    }
}
impl<T> Drop for SessionOutput<T> {
    fn drop(&mut self) {
        if !self.ends.output.load(Ordering::Acquire) {
            cancel_unsettled(&self.control);
        }
    }
}

impl<O: OperationDeclaration> Completion<O> {
    pub fn id(&self) -> OperationInstanceId {
        self.instance.id()
    }

    pub fn generation(&self) -> OperationGeneration {
        self.instance.generation()
    }

    pub fn status(&self) -> OperationStatus {
        self.instance.status()
    }

    pub fn cancel(&self) {
        self.instance.cancel();
    }

    pub async fn settle(&self) -> Result<()> {
        self.instance.settle().await
    }

    pub async fn join(mut self) -> Result<crate::operations::Response<O::Response>> {
        if !self.ends.input.load(Ordering::Acquire) || !self.ends.output.load(Ordering::Acquire) {
            return Err(Error::InvalidInput {
                field: "operation stream completion",
                reason: "finish input and drain output before joining; use cancel_and_settle to abandon",
            });
        }
        let result = (&mut self.instance).await;
        self.delivered = true;
        result
    }

    pub async fn cancel_and_settle(mut self) -> Result<()> {
        self.instance.cancel();
        let result = self.instance.settle().await;
        self.delivered = true;
        result
    }
}
impl<O: OperationDeclaration> Drop for Completion<O> {
    fn drop(&mut self) {
        if !self.delivered {
            cancel_unsettled(&self.instance.control);
        }
    }
}

impl OperationManager {
    pub fn upload<O: crate::operations::OperationDeclaration>(&self, request: O, settings: StreamSettings) -> Result<Upload<O>> {
        upload(request, settings, |request, input| self.submit_endpoints::<O, _>(request, input))
    }

    pub fn download<O: crate::operations::OperationDeclaration>(&self, request: O, settings: StreamSettings) -> Result<Download<O>> {
        download(request, settings, |request, output| self.submit_endpoints::<O, _>(request, output))
    }

    pub fn exchange<O: crate::operations::OperationDeclaration>(
        &self, request: O, input: StreamSettings, output: StreamSettings,
    ) -> Result<Exchange<O>> {
        exchange(request, input, output, |request, endpoints| self.submit_endpoints::<O, _>(request, endpoints))
    }
}

impl super::Context<'_> {
    pub fn upload<O: crate::operations::OperationDeclaration>(&self, request: O, settings: StreamSettings) -> Result<Upload<O>> {
        upload(request, settings, |request, input| self.submit_endpoints::<O, _>(request, input))
    }

    pub fn download<O: crate::operations::OperationDeclaration>(&self, request: O, settings: StreamSettings) -> Result<Download<O>> {
        download(request, settings, |request, output| self.submit_endpoints::<O, _>(request, output))
    }

    pub fn exchange<O: crate::operations::OperationDeclaration>(
        &self, request: O, input: StreamSettings, output: StreamSettings,
    ) -> Result<Exchange<O>> {
        exchange(request, input, output, |request, endpoints| self.submit_endpoints::<O, _>(request, endpoints))
    }
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
impl super::Caller {
    pub fn upload<O: crate::operations::OperationDeclaration>(&self, request: O, settings: StreamSettings) -> Result<Upload<O>> {
        upload(request, settings, |request, input| self.submit_endpoints::<O, _>(request, input))
    }

    pub fn download<O: crate::operations::OperationDeclaration>(&self, request: O, settings: StreamSettings) -> Result<Download<O>> {
        download(request, settings, |request, output| self.submit_endpoints::<O, _>(request, output))
    }

    pub fn exchange<O: crate::operations::OperationDeclaration>(
        &self, request: O, input: StreamSettings, output: StreamSettings,
    ) -> Result<Exchange<O>> {
        exchange(request, input, output, |request, endpoints| self.submit_endpoints::<O, _>(request, endpoints))
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn upload<O: crate::operations::OperationDeclaration>(
    request: O, settings: StreamSettings,
    submit: impl FnOnce(
        O,
        StreamInput<<<O as OperationDeclaration>::Input as crate::operations::Direction>::Item>,
    ) -> Result<OperationInstance<O>>,
) -> Result<Upload<O>> {
    let (sender, input) = stream(settings)?;
    let instance = submit(request, input)?;
    let ends = Arc::new(Ends { input: AtomicBool::new(false), output: AtomicBool::new(true) });
    Ok(Session {
        input: SessionInput { sender: Some(sender), ends: ends.clone(), control: instance.control.clone() },
        output: (),
        completion: Completion { instance, ends, delivered: false },
    })
}
fn download<O: crate::operations::OperationDeclaration>(
    request: O, settings: StreamSettings,
    submit: impl FnOnce(
        O,
        StreamOutput<<<O as OperationDeclaration>::Output as crate::operations::Direction>::Item>,
    ) -> Result<OperationInstance<O>>,
) -> Result<Download<O>> {
    let (output, receiver) = stream(settings)?;
    let instance = submit(request, output)?;
    let ends = Arc::new(Ends { input: AtomicBool::new(true), output: AtomicBool::new(false) });
    Ok(Session {
        input: (),
        output: SessionOutput { receiver, ends: ends.clone(), control: instance.control.clone() },
        completion: Completion { instance, ends, delivered: false },
    })
}
fn exchange<O: crate::operations::OperationDeclaration>(
    request: O, input: StreamSettings, output: StreamSettings,
    submit: impl FnOnce(
        O,
        (
            StreamInput<<<O as OperationDeclaration>::Input as crate::operations::Direction>::Item>,
            StreamOutput<<<O as OperationDeclaration>::Output as crate::operations::Direction>::Item>,
        ),
    ) -> Result<OperationInstance<O>>,
) -> Result<Exchange<O>> {
    let (sender, input) = stream(input)?;
    let (output, receiver) = stream(output)?;
    let instance = submit(request, (input, output))?;
    let ends = Arc::new(Ends { input: AtomicBool::new(false), output: AtomicBool::new(false) });
    Ok(Session {
        input: SessionInput { sender: Some(sender), ends: ends.clone(), control: instance.control.clone() },
        output: SessionOutput { receiver, ends: ends.clone(), control: instance.control.clone() },
        completion: Completion { instance, ends, delivered: false },
    })
}
fn cancel_unsettled(control: &Instance) {
    if control.status().phase() != Phase::Settled {
        control.cancel();
    }
}
async fn ended(control: &Instance) -> Error {
    let mut status = control.settlement();
    loop {
        let current = status.borrow_and_update().clone();
        if current.cancellation_requested() {
            return Refusal::Cancelled.into();
        }
        if current.outcome().is_some() {
            return Error::NotReady { domain: "operation completed" };
        }
        if status.changed().await.is_err() {
            return Refusal::Unavailable.into();
        }
    }
}

/* === Tests === */
