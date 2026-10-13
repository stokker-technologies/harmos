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
pub use crate::config::StreamSettings;

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
    receiver: mpsc::Receiver<StreamItem<T>>,
    transferred: Arc<AtomicBool>,
}

pub struct StreamItem<T> {
    value: T,
    _bytes: OwnedSemaphorePermit,
    delivery: Option<OwnedSemaphorePermit>,
}

pub struct RequestClaims {
    _original: OwnedSemaphorePermit,
    _delivery: Option<OwnedSemaphorePermit>,
}

pub struct StreamPermit<'a, T, E = Error> {
    slot: mpsc::Permit<'a, StreamItem<T>>,
    output: &'a StreamOutput<T, E>,
    claim: OwnedSemaphorePermit,
}

/* === Enums === */
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
    #[error("operation stream decode failed: {0}")]
    Codec(#[source] crate::MessageError),
}

/* === Traits === */

/* === Implementations === */
impl StreamSettings {
    pub fn validate(&self) -> Result<()> {
        if self.bytes.get() > Semaphore::MAX_PERMITS
            || self.bytes.get() > u32::MAX as usize
            || self.items.get() > Semaphore::MAX_PERMITS
        {
            return Err(Error::Settings);
        }
        Ok(())
    }
}

impl<T: Send + 'static> super::EndpointSet for StreamInput<T> {
    fn transferred(&self) -> bool {
        self.transferred.load(Ordering::Acquire)
    }
    fn item_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (Some(std::any::TypeId::of::<T>()), None)
    }
}
impl<T: Send + 'static, E: 'static> super::EndpointSet for StreamOutput<T, E> {
    fn transferred(&self) -> bool {
        self.transferred.load(Ordering::Acquire)
    }
    fn item_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        (None, Some(std::any::TypeId::of::<T>()))
    }
}
impl<T: Send + 'static> super::InputEndpoint for StreamInput<T> {
    fn transferred(&self) -> bool {
        self.transferred.load(Ordering::Acquire)
    }
    type Item = T;
}
impl<T: Send + 'static, E: 'static> super::OutputEndpoint for StreamOutput<T, E> {
    fn transferred(&self) -> bool {
        self.transferred.load(Ordering::Acquire)
    }
    type Item = T;
}

impl<T> StreamItem<T> {
    #[cfg(feature = "grpc")]
    pub fn project<A: MemorySize, B: MemorySize>(
        self,
        select: fn(T) -> std::result::Result<A, B>,
    ) -> Result<std::result::Result<StreamItem<A>, StreamItem<B>>> {
        let value = select(self.value);
        let bytes = match &value {
            Ok(value) => value.memory_size(),
            Err(value) => value.memory_size(),
        };
        if bytes > self._bytes.num_permits()
            || self
                .delivery
                .as_ref()
                .is_some_and(|claim| bytes > claim.num_permits())
        {
            return Err(Error::Construction);
        }
        Ok(match value {
            Ok(value) => Ok(StreamItem {
                value,
                _bytes: self._bytes,
                delivery: self.delivery,
            }),
            Err(value) => Err(StreamItem {
                value,
                _bytes: self._bytes,
                delivery: self.delivery,
            }),
        })
    }

    pub fn into_request(self) -> (T, RequestClaims) {
        (
            self.value,
            RequestClaims {
                _original: self._bytes,
                _delivery: self.delivery,
            },
        )
    }
}

impl<T: MemorySize> StreamItem<T> {
    pub fn charged(value: T, bytes: OwnedSemaphorePermit) -> Result<Self> {
        let item = Self {
            value,
            _bytes: bytes,
            delivery: None,
        };
        if item.value.memory_size() > item._bytes.num_permits() {
            return Err(Error::Construction);
        }
        Ok(item)
    }
}

impl<T> Deref for StreamItem<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> StreamInput<T> {
    pub fn into_receiver(self) -> mpsc::Receiver<StreamItem<T>> {
        self.receiver
    }

    pub fn try_recv(&mut self) -> std::result::Result<StreamItem<T>, mpsc::error::TryRecvError> {
        self.receiver.try_recv()
    }

    pub async fn recv(&mut self) -> Option<StreamItem<T>> {
        self.receiver.recv().await
    }

    pub fn close(&mut self) {
        self.receiver.close();
    }
}

impl<T, E> StreamOutput<T, E> {
    pub(crate) fn transfer_flag(&self) -> Arc<AtomicBool> {
        self.transferred.clone()
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
            acquiring
                .as_mut()
                .poll(cx)
                .map(|result| result.map_err(|_| closed()))
        })
        .await?;
        if self.sender.is_closed() {
            return Err(closed().into());
        }
        Ok(StreamPermit {
            slot,
            output: self,
            claim,
        })
    }
}

impl<T: MemorySize, E: From<Error>> StreamOutput<T, E> {
    pub async fn send_item(&self, mut item: StreamItem<T>) -> Result<(), E> {
        let original = Arc::ptr_eq(&self.budget, item._bytes.semaphore());
        let current = item
            .delivery
            .as_ref()
            .is_some_and(|claim| Arc::ptr_eq(&self.budget, claim.semaphore()));
        if original || current {
            let slot = self.sender.reserve().await.map_err(|_| closed())?;
            if original {
                item.delivery.take();
            }
            self.transferred.store(true, Ordering::Release);
            slot.send(item);
            return Ok(());
        }

        let permit = self.reserve(item.memory_size()).await?;
        item.delivery = Some(permit.claim);
        self.transferred.store(true, Ordering::Release);
        permit.slot.send(item);
        Ok(())
    }

    pub async fn send_with(
        &self,
        bytes: usize,
        make: impl FnOnce() -> Result<T, E>,
    ) -> Result<(), E> {
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

impl<T: Message + MemorySize, E: From<Error>> StreamOutput<T, E> {
    pub async fn decode(&self, encoded: &[u8]) -> Result<(), E> {
        let bytes = T::decoding_memory(encoded).map_err(decoding)?;
        self.send_with(bytes, || {
            T::decode(encoded).map_err(|error| decoding(error).into())
        })
        .await
    }
}

#[cfg(feature = "grpc")]
impl<T: crate::Protobuf + MemorySize, E: From<Error>> StreamOutput<T, E> {
    pub async fn decode_protobuf(&self, encoded: &[u8]) -> Result<(), E> {
        let bytes = T::decoding_memory(encoded).map_err(decoding)?;
        self.send_with(bytes, || {
            T::decode(encoded).map_err(|error| decoding(error).into())
        })
        .await
    }
}

/* === Public Functions === */
pub fn stream<T>(settings: StreamSettings) -> Result<(StreamOutput<T>, StreamInput<T>)> {
    channel::<T, Error>(settings)
}

pub fn channel<T, E: From<Error>>(
    settings: StreamSettings,
) -> Result<(StreamOutput<T, E>, StreamInput<T>), E> {
    let bytes = settings.bytes.get();
    settings.validate()?;
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
            receiver,
            transferred,
        },
    ))
}

/* === Private Functions === */
fn closed() -> Error {
    Error::Closed
}
fn decoding(source: crate::MessageError) -> Error {
    Error::Codec(source)
}

/* === Tests === */
