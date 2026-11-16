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
    columns::Columns,
    operation::{Handler, LocalFuture},
};
use harmos_protocol::model::StreamDeclaration;
use harmos_protocol::plugin::Window as WireWindow;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Stream<T: Message> {
    pub(crate) definition: StreamDefinition,
    marker: PhantomData<fn() -> T>,
}

/// Answers Each Granted Window With Its Samples as Columns.
pub struct Observer {
    pub(crate) stream: String,
    pub(crate) handler: Arc<dyn Handler>,
    pub(crate) columns: StdResult<Columns, String>,
    pub(crate) interval: Option<Duration>,
    pub(crate) interval_setting: String,
    pub(crate) window_setting: String,
}

/// One Granted Window: `count` Samples, the First at `start`, Each One `period` Later.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub start: Timestamp,
    pub period: Duration,
    pub count: u64,
    /// The One-Based Sequence of the Window's First Sample Within Its Samples Call.
    pub sequence: u64,
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

/// Calls a One-Sample Observer Once per Sample of the Window, Then Transposes the Rows.
struct Observe<T, H> {
    handler: H,
    columns: Option<Columns>,
    marker: PhantomData<fn() -> T>,
}
/// Calls a Window Observer Once per Window.
struct ObserveWindow<C, H> {
    handler: H,
    marker: PhantomData<fn() -> C>,
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
            columns: self.columns.clone(),
            interval: self.interval,
            interval_setting: self.interval_setting.clone(),
            window_setting: self.window_setting.clone(),
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
    /// Observe One Sample per Call; the SDK Calls It for Each Sample of a Window, in Order.
    pub fn new<T, H>(stream: &Stream<T>, handler: H) -> Self
    where
        T: Message,
        H: for<'a> AsyncFn(&'a Context) -> Result<T> + Send + Sync + 'static,
    {
        let columns = Columns::of::<T>();
        Self {
            stream: stream.definition.declaration.id.clone(),
            handler: Arc::new(Observe {
                handler,
                columns: columns.clone().ok(),
                marker: PhantomData,
            }),
            columns,
            interval: None,
            interval_setting: String::new(),
            window_setting: String::new(),
        }
    }

    /// Observe a Whole Window per Call; `C` Repeats Every Field of `T` With Its Number and Type.
    pub fn window<T, C, H>(stream: &Stream<T>, handler: H) -> Self
    where
        T: Message,
        C: Message,
        H: for<'a> AsyncFn(&'a Context, &'a Window) -> Result<C> + Send + Sync + 'static,
    {
        let columns = Columns::of::<T>().and_then(|columns| columns.check::<C>().map(|()| columns));
        Self {
            stream: stream.definition.declaration.id.clone(),
            handler: Arc::new(ObserveWindow { handler, marker: PhantomData }),
            columns,
            interval: None,
            interval_setting: String::new(),
            window_setting: String::new(),
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

    /// Name a Plugin Duration Setting That Overrides the Host's Window Length for This Stream.
    pub fn window_setting(mut self, setting: impl Into<String>) -> Self {
        self.window_setting = setting.into();
        self
    }
}

impl Window {
    pub(crate) fn decode(payload: &[u8]) -> Result<Self> {
        let window = <WireWindow as prost::Message>::decode(payload).map_err(|error| crate::Error::Invalid(error.to_string()))?;
        Ok(Self {
            start: window.start.unwrap_or_default(),
            period: window.period.unwrap_or_default(),
            count: window.count,
            sequence: window.sequence,
        })
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
    fn execute<'a>(&'a self, context: &'a Context, payload: Vec<u8>) -> LocalFuture<'a, Result<Vec<u8>>> {
        Box::pin(async move {
            let window = Window::decode(&payload)?;
            let columns = self.columns.as_ref().ok_or_else(|| crate::Error::Invalid("Answer Sample Window: the row has no columns".into()))?;
            let mut sample = context.clone();
            let mut rows = Vec::with_capacity(usize::try_from(window.count).unwrap_or_default());
            for index in 0..window.count {
                sample.sequence = window.sequence + index;
                rows.push((self.handler)(&sample).await?.encode_to_vec());
            }
            Ok(prost::Message::encode_to_vec(&columns.transpose(&rows)?))
        })
    }
}

impl<C, H> Handler for ObserveWindow<C, H>
where
    C: Message,
    H: for<'a> AsyncFn(&'a Context, &'a Window) -> Result<C> + Send + Sync + 'static,
{
    fn validate(&self, _: &[u8]) -> StdResult<(), Status> {
        Ok(())
    }
    fn execute<'a>(&'a self, context: &'a Context, payload: Vec<u8>) -> LocalFuture<'a, Result<Vec<u8>>> {
        Box::pin(async move {
            let window = Window::decode(&payload)?;
            Ok((self.handler)(context, &window).await?.encode_to_vec())
        })
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
