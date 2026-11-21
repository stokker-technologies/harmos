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
use std::{
    collections::BTreeMap,
    marker::PhantomData,
    mem, result,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

// Third Party Imports
use prost_types::{Duration, FileDescriptorSet, Timestamp};
use tokio::sync::{self, mpsc::Sender};
use tonic::Status;

// Local Imports
use crate::{
    Context, Message, Result,
    columns::Columns,
    operation::{Handler, LocalFuture},
};
use harmos_protocol::model::StreamDeclaration;
use harmos_protocol::plugin::{self, CollectionBatch, CollectionRows};
use harmos_protocol::transfer;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Stream<T: Message> {
    pub(crate) definition: StreamDefinition,
    marker: PhantomData<fn() -> T>,
}

/// Answers Each Granted Window With Its Samples as Columns.
#[derive(Clone)]
pub struct Observer {
    pub(crate) stream: String,

    #[cfg_attr(
        target_arch = "wasm32",
        allow(dead_code, reason = "guest metadata rejects observers; native Samples dispatch owns this handler")
    )]
    pub(crate) handler: Arc<dyn Handler>,

    pub(crate) columns: Option<result::Result<Columns, String>>,
    pub(crate) interval: Option<Duration>,
    pub(crate) interval_setting: String,
    pub(crate) window_setting: String,
    pub(crate) members: Arc<Mutex<BTreeMap<String, String>>>,
    pub(crate) lifecycle: Arc<Lifecycle>,
}

/// One Handler Publishing Zero or More Rows to Each Declared Stream per Tick.
pub struct Collection {
    pub(crate) observer: Observer,
    pub(crate) streams: Vec<StreamDefinition>,
}

/// An Owned Typed Publication Handle for One Collection Tick.
#[derive(Clone)]
pub struct CollectionOutput {
    members: Arc<Mutex<BTreeMap<String, String>>>,
    tick: Arc<Mutex<Tick>>,
}

pub(crate) struct Lifecycle {
    #[cfg_attr(
        target_arch = "wasm32",
        allow(dead_code, reason = "guest metadata rejects observers; native Samples demands own this count")
    )]
    pub(crate) active: sync::Mutex<usize>,

    pub(crate) completed: Arc<AtomicBool>,
    pub(crate) start: Option<Arc<dyn Handler>>,
    pub(crate) stop: Option<Arc<dyn Handler>>,
}

struct Tick {
    rows: BTreeMap<String, Vec<Vec<u8>>>,
    bytes: usize,
    failed: Option<Status>,
    closed: bool,
}

struct TickScope(Arc<Mutex<Tick>>);

struct Collect<H> {
    handler: H,
    members: Arc<Mutex<BTreeMap<String, String>>>,
}

struct Hook<H> {
    handler: H,
    completed: Arc<AtomicBool>,
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

#[derive(Clone)]
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

impl<T: Message> Clone for Stream<T> {
    fn clone(&self) -> Self {
        Self {
            definition: self.definition.clone(),
            marker: PhantomData,
        }
    }
}

impl Lifecycle {
    fn new() -> Self {
        Self {
            active: sync::Mutex::new(0),
            completed: Arc::new(AtomicBool::new(false)),
            start: None,
            stop: None,
        }
    }
}

impl Drop for TickScope {
    fn drop(&mut self) {
        // Cancellation and Panics Discard the Entire Tick, Including a Poisoned Encoder's Buffer.
        let mut tick = self.0.lock().unwrap_or_else(|error| error.into_inner());
        tick.closed = true;
        tick.rows.clear();
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
            columns: Some(columns),
            interval: None,
            interval_setting: String::new(),
            window_setting: String::new(),
            members: Arc::new(Mutex::new(BTreeMap::new())),
            lifecycle: Arc::new(Lifecycle::new()),
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
            columns: Some(columns),
            interval: None,
            interval_setting: String::new(),
            window_setting: String::new(),
            members: Arc::new(Mutex::new(BTreeMap::new())),
            lifecycle: Arc::new(Lifecycle::new()),
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

impl Collection {
    pub fn operation<H>(handler: H) -> Self
    where
        H: for<'a> AsyncFn(&'a Context, CollectionOutput) -> Result<()> + Send + Sync + 'static,
    {
        let members = Arc::new(Mutex::new(BTreeMap::new()));
        Self {
            observer: Observer {
                stream: String::new(),
                handler: Arc::new(Collect { handler, members: members.clone() }),
                columns: None,
                interval: None,
                interval_setting: String::new(),
                window_setting: String::new(),
                members,
                lifecycle: Arc::new(Lifecycle::new()),
            },
            streams: Vec::new(),
        }
    }

    pub fn stream<T: Message>(mut self, stream: Stream<T>) -> Self {
        self.observer
            .members
            .lock()
            .expect("collection members lock is healthy")
            .insert(stream.definition.declaration.id.clone(), T::name());
        self.streams.push(stream.definition);
        self
    }

    pub fn every(mut self, interval: Duration) -> Self {
        self.observer.interval = Some(interval);
        self
    }

    pub fn interval_setting(mut self, setting: impl Into<String>) -> Self {
        self.observer.interval_setting = setting.into();
        self
    }

    pub fn on_start<H>(mut self, hook: H) -> Self
    where
        H: for<'a> AsyncFn(&'a Context) -> Result<()> + Send + Sync + 'static,
    {
        let lifecycle = Arc::get_mut(&mut self.observer.lifecycle).expect("unregistered collection");
        lifecycle.start = Some(Arc::new(Hook {
            handler: hook,
            completed: lifecycle.completed.clone(),
        }));
        self
    }

    pub fn on_stop<H>(mut self, hook: H) -> Self
    where
        H: for<'a> AsyncFn(&'a Context) -> Result<()> + Send + Sync + 'static,
    {
        let lifecycle = Arc::get_mut(&mut self.observer.lifecycle).expect("unregistered collection");
        lifecycle.stop = Some(Arc::new(Hook {
            handler: hook,
            completed: lifecycle.completed.clone(),
        }));
        self
    }
}

impl CollectionOutput {
    /// Publish a Registered Member's Typed Row; Errors Are Returned by the Handler's Completion.
    pub fn publish<T: Message>(&self, stream: &Stream<T>, value: T) {
        let id = &stream.definition.declaration.id;
        let mut tick = self.tick.lock().expect("collection output lock is healthy");
        if tick.closed || tick.failed.is_some() {
            return;
        }
        if self.members.lock().expect("collection members lock is healthy").get(id) != Some(&T::name()) {
            tick.failed = Some(Status::invalid_argument(format!(
                "Publish Collection Row `{id}`: select a declared member with its registered message type"
            )));
            return;
        }
        let row = value.encode_to_vec();
        // Include the Row, Group and Response Framing Before Retaining Its Bytes.
        let bytes = tick.bytes.saturating_add(row.len()).saturating_add(id.len()).saturating_add(32);
        if bytes > transfer::default_message_bytes().get() {
            tick.failed = Some(Status::resource_exhausted(format!(
                "Publish Collection Row `{id}`: reduce rows per tick to fit the carrier message ceiling"
            )));
            return;
        }
        tick.bytes = bytes;
        tick.rows.entry(id.clone()).or_default().push(row);
    }
}

impl<H> Handler for Collect<H>
where
    H: for<'a> AsyncFn(&'a Context, CollectionOutput) -> Result<()> + Send + Sync + 'static,
{
    fn validate(&self, _: &[u8]) -> result::Result<(), Status> {
        Ok(())
    }

    fn execute<'a>(&'a self, context: &'a Context, payload: Vec<u8>) -> LocalFuture<'a, Result<Vec<u8>>> {
        Box::pin(async move {
            let window = Window::decode(&payload)?;
            let output = context.samples.as_ref().expect("collection window output");
            let mut sample = context.clone();
            for offset in 0..window.count {
                sample.sequence = window.sequence + offset;
                let tick = Arc::new(Mutex::new(Tick {
                    rows: BTreeMap::new(),
                    bytes: 32,
                    failed: None,
                    closed: false,
                }));
                let publisher = CollectionOutput {
                    members: self.members.clone(),
                    tick: tick.clone(),
                };
                let _tick = TickScope(tick.clone());
                let result = (self.handler)(&sample, publisher).await;
                let rows = {
                    let mut tick = tick.lock().expect("collection output lock is healthy");
                    tick.closed = true;
                    result?;
                    if let Some(error) = tick.failed.take() {
                        return Err(error.into());
                    }
                    mem::take(&mut tick.rows)
                };
                let batch = CollectionBatch {
                    offset,
                    last: offset + 1 == window.count,
                    streams: rows.into_iter().map(|(stream, rows)| CollectionRows { stream, rows }).collect(),
                };
                let bytes = prost::Message::encode_to_vec(&batch);
                output.send((bytes, None)).await.map_err(|_| Status::cancelled("Collection Demand Closed"))?;
            }
            Ok(Vec::new())
        })
    }
}

impl<H> Handler for Hook<H>
where
    H: for<'a> AsyncFn(&'a Context) -> Result<()> + Send + Sync + 'static,
{
    fn validate(&self, _: &[u8]) -> result::Result<(), Status> {
        Ok(())
    }
    fn execute<'a>(&'a self, context: &'a Context, _: Vec<u8>) -> LocalFuture<'a, Result<Vec<u8>>> {
        Box::pin(async move {
            (self.handler)(context).await?;
            self.completed.store(true, Ordering::Release);
            Ok(Vec::new())
        })
    }
}

impl Window {
    pub(crate) fn decode(payload: &[u8]) -> Result<Self> {
        let window = <plugin::Window as prost::Message>::decode(payload).map_err(|error| crate::Error::Invalid(error.to_string()))?;
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
    fn validate(&self, _: &[u8]) -> result::Result<(), Status> {
        Ok(())
    }
    fn execute<'a>(&'a self, context: &'a Context, payload: Vec<u8>) -> LocalFuture<'a, Result<Vec<u8>>> {
        Box::pin(async move {
            let window = Window::decode(&payload)?;
            let columns =
                self.columns.as_ref().ok_or_else(|| crate::Error::Invalid("Answer Sample Window: the row has no columns".into()))?;
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
    fn validate(&self, _: &[u8]) -> result::Result<(), Status> {
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
    fn validate(&self, _: &[u8]) -> result::Result<(), Status> {
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

pub(crate) fn validate_timestamp(at: Timestamp) -> result::Result<(), Status> {
    if at.nanos < 0 || at.nanos >= 1_000_000_000 || !(-62_135_596_800..=253_402_300_799).contains(&at.seconds) {
        return Err(Status::invalid_argument("Invalid Timestamp"));
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
