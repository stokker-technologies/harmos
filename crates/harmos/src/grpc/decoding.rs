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
use std::{marker::PhantomData, num::NonZeroUsize, sync::Arc};

// Third Party Imports
use bytes::Buf;
use tokio::sync::Semaphore;
use tonic::{Status, codec::DecodeBuf};

// Local Imports
use crate::{MemorySize, Protobuf, operations::StreamItem};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy, Debug)]
pub struct DecodeSettings {
    pub message_bytes: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
}

pub struct Decoder<T> {
    settings: DecodeSettings,
    budget: Arc<Semaphore>,
    failure: Option<Status>,
    message: PhantomData<fn() -> T>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl DecodeSettings {
    pub(super) fn validate(&self) -> Result<(), Status> {
        if self.message_bytes.get() > u32::MAX as usize
            || self.retained_bytes.get() > u32::MAX as usize
            || self.retained_bytes.get() > Semaphore::MAX_PERMITS
        {
            return Err(Status::invalid_argument(
                "gRPC decode limit exceeds supported capacity",
            ));
        }
        Ok(())
    }
}

impl<T: Protobuf + MemorySize + 'static> Decoder<T> {
    pub(super) fn message_limit(&self) -> usize {
        self.settings.message_bytes.get()
    }

    pub(super) fn fork(&self) -> Self {
        Self {
            settings: self.settings,
            budget: self.budget.clone(),
            failure: None,
            message: PhantomData,
        }
    }

    pub fn new(settings: DecodeSettings) -> Result<Self, Status> {
        settings.validate()?;
        Ok(Self {
            settings,
            budget: Arc::new(Semaphore::new(settings.retained_bytes.get())),
            failure: None,
            message: PhantomData,
        })
    }

    pub fn request<B>(self, body: B) -> tonic::Streaming<StreamItem<T>>
    where
        B: tonic::codegen::Body + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let limit = self.settings.message_bytes.get();
        tonic::Streaming::new_request(self, body, None, Some(limit))
    }

    fn message(&mut self, encoded: &[u8]) -> Result<StreamItem<T>, Status> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = self.decode_message(encoded);
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }

    fn decode_message(&self, encoded: &[u8]) -> Result<StreamItem<T>, Status> {
        if encoded.len() > self.settings.message_bytes.get() {
            return Err(Status::resource_exhausted(
                "gRPC message exceeds wire ceiling",
            ));
        }
        let bytes = T::decoding_memory(encoded)
            .map_err(|error| Status::invalid_argument(error.to_string()))?
            .max(std::mem::size_of::<T>())
            .max(1);
        if bytes > self.settings.retained_bytes.get() {
            return Err(Status::resource_exhausted(
                "gRPC decoded message exceeds byte ceiling",
            ));
        }
        let claim = self
            .budget
            .clone()
            .try_acquire_many_owned(bytes as u32)
            .map_err(|_| Status::resource_exhausted("gRPC decoded items retain byte capacity"))?;
        let value =
            T::decode(encoded).map_err(|error| Status::invalid_argument(error.to_string()))?;
        StreamItem::charged(value, claim).map_err(|error| Status::internal(error.to_string()))
    }
}

impl<T: Protobuf + MemorySize + 'static> tonic::codec::Decoder for Decoder<T> {
    type Item = StreamItem<T>;
    type Error = Status;

    fn decode(&mut self, source: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Status> {
        let length = source.remaining();
        let value = self.message(source.chunk())?;
        source.advance(length);
        Ok(Some(value))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
