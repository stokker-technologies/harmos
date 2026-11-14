/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Rust Plugin SDK

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::result::Result as StdResult;
use std::{marker::PhantomData, sync::Arc};

// Third Party Imports
use prost_types::{Duration, FileDescriptorSet, Timestamp};
use tokio::sync::mpsc::Sender;
use tonic::Status;

// Local Imports
use crate::{
    Context, Message, Result,
    operation::{Handler, LocalFuture},
};
use harmos_protocol::model::StreamDeclaration;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Stream<T: Message> {
    pub(crate) definition: StreamDefinition,
    marker: PhantomData<fn() -> T>,
}

pub struct Observer {
    pub(crate) stream: String,
    pub(crate) handler: Arc<dyn Handler>,
    pub(crate) interval: Option<Duration>,
    pub(crate) interval_setting: String,
}

pub struct Publisher<T: Message> {
    output: Sender<(Vec<u8>, Option<Timestamp>)>,
    marker: PhantomData<fn() -> T>,
}

pub(crate) struct StreamDefinition {
    pub declaration: StreamDeclaration,
    pub types: FileDescriptorSet,
    pub producer: Option<Arc<dyn Handler>>,
}

struct Observe<T, H> {
    handler: H,
    marker: PhantomData<fn() -> T>,
}
struct Produce<T, H> {
    handler: H,
    marker: PhantomData<fn() -> T>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Clone for Observer {
    fn clone(&self) -> Self {
        Self {
            stream: self.stream.clone(),
            handler: Arc::clone(&self.handler),
            interval: self.interval,
            interval_setting: self.interval_setting.clone(),
        }
    }
}

impl<T: Message> Stream<T> {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            definition: StreamDefinition {
                declaration: StreamDeclaration {
                    id: id.into(),
                    row: T::name(),
                    ..Default::default()
                },
                types: T::types(),
                producer: None,
            },
            marker: PhantomData,
        }
    }

    pub fn publish_on<H>(mut self, handler: H) -> Self
    where
        H: for<'a> AsyncFn(&'a Context, Publisher<T>) -> Result<()> + Send + Sync + 'static,
    {
        self.definition.producer = Some(Arc::new(Produce { handler, marker: PhantomData }));
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.definition.declaration.description = description.into();
        self
    }
}

impl Observer {
    pub fn new<T, H>(stream: &Stream<T>, handler: H) -> Self
    where
        T: Message,
        H: for<'a> AsyncFn(&'a Context) -> Result<T> + Send + Sync + 'static,
    {
        Self {
            stream: stream.definition.declaration.id.clone(),
            handler: Arc::new(Observe { handler, marker: PhantomData }),
            interval: None,
            interval_setting: String::new(),
        }
    }

    pub fn every(mut self, interval: Duration) -> Self {
        self.interval = Some(interval);
        self
    }

    pub fn interval_setting(mut self, setting: impl Into<String>) -> Self {
        self.interval_setting = setting.into();
        self
    }
}

impl<T: Message> Publisher<T> {
    pub async fn publish(&self, value: T) -> Result<()> {
        self.publish_at(value, None).await
    }

    pub async fn publish_at(&self, value: T, at: Option<Timestamp>) -> Result<()> {
        if let Some(at) = at {
            validate_timestamp(at)?;
        }
        self.output.send((value.encode_to_vec(), at)).await.map_err(|_| Status::cancelled("Demand Closed").into())
    }
}

impl<T, H> Handler for Observe<T, H>
where
    T: Message,
    H: for<'a> AsyncFn(&'a Context) -> Result<T> + Send + Sync + 'static,
{
    fn validate(&self, _: &[u8]) -> StdResult<(), Status> {
        Ok(())
    }
    fn execute<'a>(&'a self, context: &'a Context, _: Vec<u8>) -> LocalFuture<'a, Result<Vec<u8>>> {
        Box::pin(async move { Ok((self.handler)(context).await?.encode_to_vec()) })
    }
}

impl<T, H> Handler for Produce<T, H>
where
    T: Message,
    H: for<'a> AsyncFn(&'a Context, Publisher<T>) -> Result<()> + Send + Sync + 'static,
{
    fn validate(&self, _: &[u8]) -> StdResult<(), Status> {
        Ok(())
    }
    fn execute<'a>(&'a self, context: &'a Context, _: Vec<u8>) -> LocalFuture<'a, Result<Vec<u8>>> {
        Box::pin(async move {
            let publisher = Publisher {
                output: context.samples.as_ref().expect("Publication Output").clone(),
                marker: PhantomData,
            };
            (self.handler)(context, publisher).await?;
            Ok(Vec::new())
        })
    }
}

/* === Public Functions === */

pub(crate) fn validate_timestamp(at: Timestamp) -> StdResult<(), Status> {
    if at.nanos < 0 || at.nanos >= 1_000_000_000 || !(-62_135_596_800..=253_402_300_799).contains(&at.seconds) {
        return Err(Status::invalid_argument("Invalid Timestamp"));
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
