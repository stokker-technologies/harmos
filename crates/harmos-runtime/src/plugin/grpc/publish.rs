/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Connection-Owned Publication Demand and Host Ticks

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos_protocol::plugin::InstanceSamplesRequest;
use harmos_protocol::plugin::InstanceSamplesResponse;
use harmos_protocol::plugin::SamplesSelection;
use harmos_protocol::plugin::instance_samples_request::Frame;

// Standard Library Imports
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error as StdError,
    sync::{Arc, Mutex},
    time::Duration,
};

// Third Party Imports
use futures_util::stream;
use tokio::sync::{Mutex as AsyncMutex, mpsc, watch};

// Local Imports
use super::{Activation, Error, PlacementSettings, model};
use crate::streams::protobuf::Row;
use crate::{
    Application,
    clock::Clock,
    streams::{ConnectionStreams, PublicationRow, StreamManager, StreamWriter},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Publications {
    close: watch::Sender<bool>,
    jobs: tokio::task::JoinSet<Result<(), Error>>,
    registration: Arc<ConnectionStreams>,
    parent: Option<watch::Receiver<bool>>,
    source: Option<Arc<std::sync::OnceLock<ScopeSource>>>,
}

pub(super) struct ScopeSource {
    pub(super) registration: Arc<ConnectionStreams>,
    pub(super) close: watch::Receiver<bool>,
    pub(super) settings: PlacementSettings,
    pub(super) operations: crate::operations::Manager,
    pub(super) scopes: Mutex<BTreeMap<String, watch::Receiver<Option<super::Outcome>>>>,
}

pub(super) struct PreparedPublications {
    declarations: Vec<harmos_operations::StreamDeclaration>,
    demands: Vec<PreparedDemand>,
}
struct PreparedDemand {
    id: String,
    output: mpsc::Sender<InstanceSamplesRequest>,
    input: tonic::Streaming<InstanceSamplesResponse>,
    row: Row,
    interval: Option<Duration>,
}

struct Demand {
    output: mpsc::Sender<InstanceSamplesRequest>,
    input: tonic::Streaming<InstanceSamplesResponse>,
    row: Row,
    interval: Option<Duration>,
    lease: Arc<StreamWriter>,
    close: watch::Receiver<bool>,
    parent: Option<watch::Receiver<bool>>,
    closed: watch::Receiver<bool>,
    operations: crate::operations::Manager,
    caller: Arc<super::CallingPlugin>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Publications {
    pub(super) async fn open<A: Application>(
        streams: &StreamManager<A>, activation: &Activation, clock: &Clock, settings: &PlacementSettings,
        operations: &crate::operations::Manager,
    ) -> Result<Self, Error> {
        let prepared = Self::prepare(streams, activation, settings, operations, false).await?;
        Self::activate(prepared, streams, activation, clock, operations, settings, false)
    }
    pub(super) async fn prepare<A: Application>(
        streams: &StreamManager<A>, activation: &Activation, settings: &PlacementSettings, operations: &crate::operations::Manager,
        replace: bool,
    ) -> Result<PreparedPublications, Error> {
        let catalog = activation.description.declarations.as_ref().expect("admitted description");
        let pool = prost_reflect::DescriptorPool::from_file_descriptor_set(catalog.types.clone().unwrap_or_default()).map_err(invalid)?;
        let mut declarations = Vec::new();
        let mut rows = Vec::new();
        for stream in &catalog.streams {
            let descriptor = pool
                .get_message_by_name(stream.row.trim_start_matches('.'))
                .ok_or_else(|| invalid("Prepare Samples: missing stream row"))?;
            let row = Row::new(descriptor)?;
            let interval = interval(stream, &activation.settings)?;
            declarations.push(harmos_operations::StreamDeclaration {
                id: stream.id.clone(),
                window: settings.stream_window.get(),
                row: row.schema.clone(),
                metadata: crate::Metadata::new(crate::Identifier::new(&stream.id).map_err(invalid)?, stream.id.clone(), None),
                row_metadata: Vec::new(),
            });
            rows.push((stream.id.clone(), row, interval));
        }
        streams.validate_connection(activation.owner(), &declarations, replace).map_err(invalid)?;
        let mut demands = Vec::new();
        for (id, row, interval) in rows {
            let (output, input) = mpsc::channel(1);
            output
                .send(InstanceSamplesRequest {
                    frame: Some(Frame::Open(SamplesSelection {
                        stream: id.clone(),
                        scope: operations.call_context().get::<crate::Scope>().map(|scope| scope.as_str().to_owned()).unwrap_or_default(),
                    })),
                })
                .await
                .map_err(invalid)?;
            let input = stream::unfold(input, |mut input| async { input.recv().await.map(|frame| (frame, input)) });
            let input =
                activation.connection.client().samples(input).await.map_err(|error| Error::Transport(error.to_string()))?.into_inner();
            demands.push(PreparedDemand { id, output, input, row, interval });
        }
        Ok(PreparedPublications { declarations, demands })
    }
    pub(super) fn activate<A: Application>(
        prepared: PreparedPublications, streams: &StreamManager<A>, activation: &Activation, clock: &Clock,
        operations: &crate::operations::Manager, settings: &PlacementSettings, replace: bool,
    ) -> Result<Self, Error> {
        let registration = Arc::new(streams.connect_publications(activation.owner(), prepared.declarations, replace).map_err(invalid)?);
        let mut publications = Self::start(prepared.demands, registration.clone(), activation, clock, operations, None);
        activation
            .publications
            .set(ScopeSource {
                registration,
                close: publications.close.subscribe(),
                settings: settings.clone(),
                operations: operations.clone(),
                scopes: Mutex::new(BTreeMap::new()),
            })
            .map_err(|_| invalid("Activate Samples: activation already has publications"))?;
        publications.source = Some(activation.publications.clone());
        Ok(publications)
    }

    pub(super) fn scoped(
        prepared: PreparedPublications, source: &ScopeSource, activation: &Activation, clock: &Clock,
        operations: &crate::operations::Manager,
    ) -> Self {
        Self::start(prepared.demands, source.registration.clone(), activation, clock, operations, Some(source.close.clone()))
    }

    fn start(
        demands: Vec<PreparedDemand>, registration: Arc<ConnectionStreams>, activation: &Activation, clock: &Clock,
        operations: &crate::operations::Manager, parent: Option<watch::Receiver<bool>>,
    ) -> Self {
        let close = watch::channel(false).0;
        let mut publications = Self {
            close,
            jobs: tokio::task::JoinSet::new(),
            registration,
            parent,
            source: None,
        };
        for prepared in demands {
            let activity = clock.activity(format!("Publish {}", prepared.id));
            let lease = publications
                .registration
                .leases()
                .iter()
                .find(|lease| lease.stream().definition().id() == prepared.id)
                .expect("staged stream lease")
                .clone();
            let close = publications.close.subscribe();
            let parent = publications.parent.clone();
            let connection = activation.connection.clone();
            let clock = clock.clone();
            let operations = operations.clone();
            let caller = Arc::new(super::CallingPlugin {
                name: activation.entry_name.clone(),
                connection: activation.connection.clone(),
                provider: activation.provider.clone(),
                description: activation.description.clone(),
            });
            publications.jobs.spawn(async move {
                let demand = Demand {
                    output: prepared.output,
                    input: prepared.input,
                    row: prepared.row,
                    interval: prepared.interval,
                    lease,
                    close,
                    parent,
                    closed: connection.closed.subscribe(),
                    operations,
                    caller,
                };
                let result = run(&clock, activity, demand).await;
                if let Err(error) = &result {
                    tracing::warn!(%error, "plugin publication ended");
                    if !matches!(error, Error::Invalid(_)) {
                        connection.closed.send_replace(true);
                    }
                }
                result
            });
        }
        publications
    }

    pub(super) async fn close(&mut self) -> Result<(), Error> {
        self.close.send_replace(true);
        let mut failure = None;
        while let Some(result) = self.jobs.join_next().await {
            if let Err(error) = result.map_err(invalid).and_then(|result| result)
                && self.parent.is_some()
            {
                failure.get_or_insert(error);
            }
        }
        if let Some(source) = self.source.as_ref().and_then(|source| source.get()) {
            let scopes = source.scopes.lock().expect("scoped publications lock is healthy").values().cloned().collect::<Vec<_>>();
            for mut scope in scopes {
                while scope.borrow_and_update().is_none() {
                    if scope.changed().await.is_err() {
                        break;
                    }
                }
                if let Some(Err(error)) = scope.borrow().as_ref() {
                    failure.get_or_insert_with(|| invalid(format!("Close Scoped Samples: {error}")));
                }
            }
            for lease in self.registration.leases() {
                lease.revoke();
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

impl Drop for Publications {
    fn drop(&mut self) {
        self.close.send_replace(true);
        self.jobs.abort_all();
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(error: impl std::fmt::Display) -> Error {
    Error::Invalid(error.to_string())
}

fn interval(stream: &model::StreamDeclaration, settings: &model::SettingValues) -> Result<Option<Duration>, Error> {
    let mut interval = stream.interval;
    if !stream.interval_setting.is_empty() {
        let value = settings
            .values
            .get(&stream.interval_setting)
            .and_then(|value| value.value.as_ref())
            .ok_or_else(|| invalid("Read Stream Interval: setting is not declared"))?;
        let model::setting_value::Value::Duration(value) = value else {
            return Err(invalid("Read Stream Interval: setting must be a duration"));
        };
        interval = Some(*value);
    }
    interval
        .map(|value| {
            if value.seconds < 0 || !(0..1_000_000_000).contains(&value.nanos) || value.seconds > 315_576_000_000 {
                return Err(invalid("Invalid Stream Interval"));
            }
            let period = Duration::new(value.seconds as u64, value.nanos as u32);
            if period.is_zero() {
                return Err(invalid("Stream Interval Must Be Positive"));
            }
            Ok(period)
        })
        .transpose()
}

fn sample_error(error: crate::Error) -> Error {
    let mut source: Option<&(dyn StdError + 'static)> = Some(&error);
    while let Some(current) = source {
        if let Some(Error::Transport(message)) = current.downcast_ref::<Error>() {
            return Error::Transport(message.clone());
        }
        source = current.source();
    }
    invalid(error)
}

async fn run(clock: &Clock, mut activity: crate::clock::Activity, demand: Demand) -> Result<(), Error> {
    let Demand {
        output,
        input,
        row,
        interval,
        lease,
        close,
        mut parent,
        closed,
        operations,
        caller,
    } = demand;
    let input = Arc::new(AsyncMutex::new(input));
    let seen = Arc::new(Mutex::new(BTreeSet::new()));
    let mut ingested_through = None;
    {
        let work = async {
            let mut sequence = 0;
            let mut next_tick = interval.map(|period| clock.now() + period);
            loop {
                let tick = if let Some(period) = interval {
                    let next = clock.sleep_until(next_tick.expect("interval streams have a next tick"));
                    let wait = async {
                        tokio::select! {
                            biased;
                            frame = async { input.lock().await.message().await } => Err(invalid(format!("Read Interval Samples Outside Tick: {frame:?}"))),
                            () = next => Ok(()),
                        }
                    };
                    activity.wait(wait).await?;
                    let reading = clock.read();
                    next_tick = Some(reading.instant + period);
                    let at = reading.date.into();
                    Some(at)
                } else {
                    None
                };
                let samples = if let Some(at) = tick {
                    activity
                        .wait(operations.call_native::<super::samples::Tick>(super::samples::Request {
                            input: input.clone(),
                            output: output.clone(),
                            caller: caller.clone(),
                            seen: seen.clone(),
                            lease: lease.clone(),
                            at,
                        }))
                        .await
                        .map_err(sample_error)?
                } else {
                    let response = input
                        .lock()
                        .await
                        .message()
                        .await
                        .map_err(|error| Error::Transport(error.to_string()))?
                        .ok_or_else(|| invalid("Read Device Samples: demand ended"))?;
                    if response.frame.is_some() {
                        return Err(invalid("Read Device Samples: child calls require an unanswered tick"));
                    }
                    response
                };
                if samples.samples.len() != 1 {
                    return Err(invalid("Expected One Sample per Message"));
                }
                let _sample = clock.activity("plugin sample ingest");
                let sample = samples.samples.into_iter().next().unwrap();
                sequence += 1;
                if sample.sequence != sequence {
                    return Err(invalid("Invalid Stream Sequence"));
                }
                if tick.is_some() && sample.at != tick {
                    return Err(invalid("Sample Time Differs from Its Tick"));
                }
                if sample
                    .at
                    .is_some_and(|at| !(-62_135_596_800..=253_402_300_799).contains(&at.seconds) || !(0..1_000_000_000).contains(&at.nanos))
                {
                    return Err(invalid("Invalid Sample Time"));
                }
                let published = tick.unwrap_or_else(|| clock.date().into());
                let mut context = serde_json::Map::new();
                if interval.is_none()
                    && let Some(at) = sample.at
                {
                    context.insert("source_time".into(), serde_json::json!({"seconds": at.seconds, "nanos": at.nanos}));
                }
                if let Some(scope) = operations.call_context().get::<crate::Scope>() {
                    context.insert("scope".into(), serde_json::json!(scope.as_str()));
                }
                let context = (!context.is_empty()).then(|| serde_json::Value::Object(context).to_string());
                let payload = row.decode(&sample.row)?;
                let receipt = lease
                    .admit_clocked_row(
                        PublicationRow {
                            payload,
                            published_unix_micros: Some(published.seconds * 1_000_000 + i64::from(published.nanos) / 1000),
                            context,
                        },
                        clock,
                    )
                    .map_err(invalid)?;
                ingested_through = Some(receipt);
                lease.ingested_at(receipt).await.map_err(invalid)?;
            }
        };
        tokio::pin!(work);
        tokio::select! {
            biased;
            () = super::disconnected(closed) => return Ok(()),
            () = super::disconnected(close.clone()) => {},
            () = async {
                match &mut parent {
                    Some(parent) => super::disconnected(parent.clone()).await,
                    None => std::future::pending().await,
                }
            } => {},
            result = &mut work => return result,
        }
    }
    if let Some(published) = ingested_through {
        lease.ingested_at(published).await.map_err(invalid)?;
    }
    drop(output);
    // Input EOF Ends Demand and Awaits the SDK's Producer/Observer Cleanup.
    while input.lock().await.message().await.map_err(|error| Error::Transport(error.to_string()))?.is_some() {}
    Ok(())
}

/* === Tests === */
