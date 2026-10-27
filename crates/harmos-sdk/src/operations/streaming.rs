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

pub use harmos_domain::config::StreamSettings;

/* === Imports === */

// Standard Library Imports
use std::{
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

// Third Party Imports
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};

// Local Imports
use super::RequestClaims;
use crate::{MemorySize, Message};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Result<T, E = Error> = std::result::Result<T, E>;

/* === Structs === */

pub struct StreamOutput<T, E = Error> {
    error: std::marker::PhantomData<fn() -> E>,
    sender: mpsc::Sender<StreamItem<T>>,
    budget: Arc<Semaphore>,
    limit: usize,
    transferred: Arc<AtomicBool>,
}

pub struct StreamInput<T> {
    receiver: tokio::sync::Mutex<mpsc::Receiver<StreamItem<T>>>,
    transferred: Arc<AtomicBool>,
}

pub struct StreamItem<T> {
    value: Payload<T>,
    _bytes: OwnedSemaphorePermit,
    delivery: Option<OwnedSemaphorePermit>,
}

pub struct StreamPermit<'a, T, E = Error> {
    slot: mpsc::Permit<'a, StreamItem<T>>,
    output: &'a StreamOutput<T, E>,
    claim: OwnedSemaphorePermit,
}

/* === Enums === */

enum Payload<T> {
    Plain(T),
    Owned { response: super::response::ErasedResponse, project: fn(&super::response::ErasedResponse) -> &T },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("stream capacity exceeds semaphore limits")]
    Settings,

    #[error("operation stream byte capacity exhausted")]
    Capacity,

    #[error("constructed value exceeds its reserved stream bytes")]
    Construction,

    #[error("operation stream input is closed")]
    Closed,

    #[error("borrowed stream payload cannot be moved out of its original owner")]
    Ownership,

    #[error("operation stream decode failed: {0}")]
    Codec(#[source] crate::MessageError),
}

/* === Traits === */

/* === Implementations === */

impl<T: Message + MemorySize + Send + 'static> super::EndpointSet for StreamInput<T> {
    fn bind_wire(limits: super::WireStreamLimits) -> Result<(Self, super::WireStreams), super::ServingError> {
        <Self as super::WireEndpoints>::bind(limits)
    }

    fn transferred(&self) -> bool {
        self.transferred.load(Ordering::Acquire)
    }
    fn item_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (Some(std::any::TypeId::of::<T>()), None)
    }
}
impl<T: Message + MemorySize + Send + 'static, E: 'static> super::EndpointSet for StreamOutput<T, E> {
    fn bind_wire(limits: super::WireStreamLimits) -> Result<(Self, super::WireStreams), super::ServingError> {
        <Self as super::WireEndpoints>::bind(limits)
    }

    fn transferred(&self) -> bool {
        self.transferred.load(Ordering::Acquire)
    }
    fn item_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (None, Some(std::any::TypeId::of::<T>()))
    }
}
impl<T: Message + MemorySize + Send + 'static> super::InputEndpoint for StreamInput<T> {
    fn bind_wire(limits: super::WireStreamLimits) -> Result<(Self, super::WireStreams), super::ServingError> {
        <Self as super::WireEndpoints>::bind(limits)
    }

    fn transferred(&self) -> bool {
        self.transferred.load(Ordering::Acquire)
    }
    type Item = T;
}
impl<T: Message + MemorySize + Send + 'static, E: 'static> super::OutputEndpoint for StreamOutput<T, E> {
    fn bind_wire(limits: super::WireStreamLimits) -> Result<(Self, super::WireStreams), super::ServingError> {
        <Self as super::WireEndpoints>::bind(limits)
    }

    fn transferred(&self) -> bool {
        self.transferred.load(Ordering::Acquire)
    }
    type Item = T;
}

impl<T> StreamItem<T> {
    #[cfg(feature = "grpc")]
    pub fn project<A: MemorySize, B: MemorySize>(
        self, select: fn(T) -> std::result::Result<A, B>,
    ) -> Result<std::result::Result<StreamItem<A>, StreamItem<B>>> {
        let Payload::Plain(value) = self.value else {
            return Err(Error::Ownership);
        };
        let value = select(value);
        let bytes = match &value {
            Ok(value) => value.memory_size(),
            Err(value) => value.memory_size(),
        };
        if bytes > self._bytes.num_permits() || self.delivery.as_ref().is_some_and(|claim| bytes > claim.num_permits()) {
            return Err(Error::Construction);
        }
        Ok(match value {
            Ok(value) => Ok(StreamItem {
                value: Payload::Plain(value),
                _bytes: self._bytes,
                delivery: self.delivery,
            }),
            Err(value) => Err(StreamItem {
                value: Payload::Plain(value),
                _bytes: self._bytes,
                delivery: self.delivery,
            }),
        })
    }

    pub fn into_request(self) -> Result<(T, RequestClaims)> {
        let Payload::Plain(value) = self.value else {
            return Err(Error::Ownership);
        };
        Ok((value, RequestClaims::retain(Arc::new((self._bytes, self.delivery)))))
    }
}

impl<T: MemorySize> StreamItem<T> {
    pub fn charged(value: T, bytes: OwnedSemaphorePermit) -> Result<Self> {
        let item = Self {
            value: Payload::Plain(value),
            _bytes: bytes,
            delivery: None,
        };
        if item.memory_size() > item._bytes.num_permits() {
            return Err(Error::Construction);
        }
        Ok(item)
    }
}

impl<T: Clone + Send + Sync + 'static> StreamItem<T> {
    // Shared Or Projected Responses Need An Owned Copy; Plain Requests Move.
    pub fn into_owned_request(self) -> Result<(T, RequestClaims)> {
        let value = match self.value {
            Payload::Plain(value) => value,
            Payload::Owned { response, .. } => response.restore::<T>().ok_or(Error::Ownership)?.into_owned(),
        };
        let claims = RequestClaims::retain(Arc::new((self._bytes, self.delivery)));
        Ok((value, claims))
    }
}

impl<T: MemorySize + Send + Sync + 'static> StreamItem<T> {
    pub fn charged_owned(value: super::Response<T>, bytes: OwnedSemaphorePermit) -> Result<Self> {
        let retained = value.memory_size().checked_add(std::mem::size_of::<Self>()).ok_or(Error::Capacity)?;
        if retained > bytes.num_permits() {
            return Err(Error::Construction);
        }
        Ok(Self {
            value: Payload::Owned {
                response: value.into_erased(),
                project: owned::<T>,
            },
            _bytes: bytes,
            delivery: None,
        })
    }
}

impl<T> Deref for StreamItem<T> {
    type Target = T;
    fn deref(&self) -> &T {
        match &self.value {
            Payload::Plain(value) => value,
            Payload::Owned { response, project } => project(response),
        }
    }
}

impl<T> StreamInput<T> {
    pub fn into_receiver(self) -> mpsc::Receiver<StreamItem<T>> {
        self.receiver.into_inner()
    }

    pub fn try_recv(&mut self) -> std::result::Result<StreamItem<T>, mpsc::error::TryRecvError> {
        self.receiver.get_mut().try_recv()
    }

    #[tracing::instrument(target = "harmos::streams", name = "streams.recv", level = "trace", skip_all)]
    pub async fn recv(&self) -> Option<StreamItem<T>> {
        self.receiver.lock().await.recv().await
    }

    pub fn close(&mut self) {
        self.receiver.get_mut().close();
    }
}

impl<T, E> StreamOutput<T, E> {
    pub(crate) fn transfer_flag(&self) -> Arc<AtomicBool> {
        self.transferred.clone()
    }

    // Wire bindings construct the channel with the portable error and hand the
    // same sender to the operation under its declared error type.
    pub(crate) fn failing_with<F>(self) -> StreamOutput<T, F> {
        StreamOutput {
            error: std::marker::PhantomData,
            sender: self.sender,
            budget: self.budget,
            limit: self.limit,
            transferred: self.transferred,
        }
    }
}

impl<T, E> Clone for StreamOutput<T, E> {
    fn clone(&self) -> Self {
        Self {
            error: std::marker::PhantomData,
            sender: self.sender.clone(),
            budget: self.budget.clone(),
            limit: self.limit,
            transferred: self.transferred.clone(),
        }
    }
}

impl<T, E: From<Error>> StreamOutput<T, E> {
    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }

    #[tracing::instrument(target = "harmos::streams", name = "streams.reserve", level = "trace", skip_all)]
    pub async fn reserve(&self, bytes: usize) -> Result<StreamPermit<'_, T, E>, E> {
        let bytes = bytes.max(std::mem::size_of::<T>()).max(1);
        if bytes > self.limit {
            return Err(Error::Capacity.into());
        }
        let slot = self.sender.reserve().await.map_err(|_| closed())?;
        // Race Closure Against Byte Admission Without Requiring An Executor.
        use std::{future::Future, task::Poll};
        let mut closing = std::pin::pin!(self.sender.closed());
        let mut acquiring = std::pin::pin!(self.budget.clone().acquire_many_owned(bytes as u32));
        let claim = std::future::poll_fn(|cx| {
            if closing.as_mut().poll(cx).is_ready() {
                return Poll::Ready(Err(closed()));
            }
            acquiring.as_mut().poll(cx).map(|result| result.map_err(|_| closed()))
        })
        .await?;
        if self.sender.is_closed() {
            return Err(closed().into());
        }
        Ok(StreamPermit { slot, output: self, claim })
    }
}

impl<T: MemorySize, E: From<Error>> StreamOutput<T, E> {
    #[tracing::instrument(target = "harmos::streams", name = "streams.send_item", level = "trace", skip_all)]
    pub async fn send_item(&self, mut item: StreamItem<T>) -> Result<(), E> {
        let original = Arc::ptr_eq(&self.budget, item._bytes.semaphore());
        let current = item.delivery.as_ref().is_some_and(|claim| Arc::ptr_eq(&self.budget, claim.semaphore()));
        if original || current {
            let slot = self.sender.reserve().await.map_err(|_| closed())?;
            if original {
                item.delivery.take();
            }
            self.transferred.store(true, Ordering::Release);
            slot.send(item);
            return Ok(());
        }

        let bytes = item
            .memory_size()
            .checked_add(match &item.value {
                Payload::Plain(_) => 0,
                Payload::Owned { .. } => std::mem::size_of::<StreamItem<T>>(),
            })
            .ok_or(Error::Capacity)?;
        let permit = self.reserve(bytes).await?;
        item.delivery = Some(permit.claim);
        self.transferred.store(true, Ordering::Release);
        permit.slot.send(item);
        Ok(())
    }

    #[tracing::instrument(target = "harmos::streams", name = "streams.send_with", level = "trace", skip_all)]
    pub async fn send_with(&self, bytes: usize, make: impl FnOnce() -> Result<T, E>) -> Result<(), E> {
        self.reserve(bytes).await?.send_with(make)
    }
}

impl<T: MemorySize, E: From<Error>> StreamPermit<'_, T, E> {
    pub fn send_with(self, make: impl FnOnce() -> Result<T, E>) -> Result<(), E> {
        if self.output.is_closed() {
            return Err(closed().into());
        }
        let value = make()?;
        let item = StreamItem::charged(value, self.claim)?;
        self.output.transferred.store(true, Ordering::Release);
        self.slot.send(item);
        Ok(())
    }
}

impl<T: MemorySize + Send + Sync + 'static, E: From<Error>> StreamOutput<T, E> {
    #[tracing::instrument(target = "harmos::streams", name = "streams.send_owned", level = "trace", skip_all)]
    pub async fn send_owned(&self, value: super::Response<T>) -> Result<(), E> {
        let bytes = value.memory_size().checked_add(std::mem::size_of::<StreamItem<T>>()).ok_or(Error::Capacity)?;
        self.reserve(bytes).await?.send_owned(value)
    }
}

impl<T: MemorySize + Send + Sync + 'static, E: From<Error>> StreamPermit<'_, T, E> {
    pub fn send_owned(self, value: super::Response<T>) -> Result<(), E> {
        if self.output.is_closed() {
            return Err(closed().into());
        }
        let item = StreamItem::charged_owned(value, self.claim)?;
        self.output.transferred.store(true, Ordering::Release);
        self.slot.send(item);
        Ok(())
    }
}

impl<T: Message + MemorySize, E: From<Error>> StreamOutput<T, E> {
    #[tracing::instrument(target = "harmos::streams", name = "streams.decode", level = "trace", skip_all)]
    pub async fn decode(&self, encoded: &[u8]) -> Result<(), E> {
        let bytes = T::decoding_memory(encoded).map_err(decoding)?;
        self.send_with(bytes, || T::decode(encoded).map_err(|error| decoding(error).into())).await
    }
}

/* === Public Functions === */

pub fn validate(settings: &StreamSettings) -> Result<()> {
    if settings.bytes.get() > Semaphore::MAX_PERMITS
        || settings.bytes.get() > u32::MAX as usize
        || settings.items.get() > Semaphore::MAX_PERMITS
    {
        return Err(Error::Settings);
    }
    Ok(())
}

pub fn stream<T>(settings: StreamSettings) -> Result<(StreamOutput<T>, StreamInput<T>)> {
    channel::<T, Error>(settings)
}

pub fn channel<T, E: From<Error>>(settings: StreamSettings) -> Result<(StreamOutput<T, E>, StreamInput<T>), E> {
    let bytes = settings.bytes.get();
    validate(&settings)?;
    let (sender, receiver) = mpsc::channel(settings.items.get());
    let transferred = Arc::new(AtomicBool::new(false));
    Ok((
        StreamOutput {
            error: std::marker::PhantomData,
            sender,
            budget: Arc::new(Semaphore::new(bytes)),
            limit: bytes,
            transferred: transferred.clone(),
        },
        StreamInput {
            receiver: tokio::sync::Mutex::new(receiver),
            transferred,
        },
    ))
}

/* === Private Functions === */

fn owned<T: Send + Sync + 'static>(response: &super::response::ErasedResponse) -> &T {
    response.value::<T>().expect("owned stream payload retains its declared type")
}

fn closed() -> Error {
    Error::Closed
}
fn decoding(source: crate::MessageError) -> Error {
    Error::Codec(source)
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_claim_clones_share_admission() {
        let budget = Arc::new(Semaphore::new(64));
        let permit = budget.clone().try_acquire_many_owned(64).unwrap();
        let item = StreamItem::charged(String::from("payload"), permit).unwrap();
        let (payload, claims) = item.into_request().unwrap();
        let retained = claims.clone();
        drop(payload);
        drop(claims);
        assert_eq!(budget.available_permits(), 0);
        drop(retained);
        assert_eq!(budget.available_permits(), 64);
    }

    #[test]
    fn owned_requests() {
        let budget = Arc::new(Semaphore::new(4096));
        let plain = String::from("plain payload");
        let pointer = plain.as_ptr();
        let claim = budget.clone().try_acquire_many_owned(4096).unwrap();
        let item = StreamItem::charged(plain, claim).unwrap();
        let (plain, claims) = item.into_owned_request().unwrap();
        assert_eq!(plain.as_ptr(), pointer);
        drop(claims);

        let unique = super::super::Response::new(String::from("unique payload"));
        let pointer = unique.as_ptr();
        let claim = budget.clone().try_acquire_many_owned(4096).unwrap();
        let item = StreamItem::charged_owned(unique, claim).unwrap();
        let (unique, claims) = item.into_owned_request().unwrap();
        assert_eq!(unique.as_ptr(), pointer);
        drop(claims);

        let shared = super::super::Response::new(String::from("shared payload"));
        let claim = budget.clone().try_acquire_many_owned(4096).unwrap();
        let item = StreamItem::charged_owned(shared.clone(), claim).unwrap();
        let (owned, claims) = item.into_owned_request().unwrap();
        assert_eq!(owned, *shared);
        assert_ne!(owned.as_ptr(), shared.as_ptr());
        drop(claims);
        assert_eq!(budget.available_permits(), 4096);
    }
}
