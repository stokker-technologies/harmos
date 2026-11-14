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

// Standard Library Imports
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
    time::Duration,
};

// Third Party Imports
use futures_util::stream;
use tokio::sync::{Mutex as AsyncMutex, mpsc, watch};

// Local Imports
use super::{Activation, Error, PlacementSettings, v1};
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
    jobs: tokio::task::JoinSet<()>,
    registration: ConnectionStreams,
}

pub(super) struct PreparedPublications {
    declarations: Vec<harmos_operations::StreamDeclaration>,
    demands: Vec<PreparedDemand>,
}
struct PreparedDemand {
    id: String,
    output: mpsc::Sender<harmos_protocol::plugin::InstanceSamplesRequest>,
    input: tonic::Streaming<harmos_protocol::plugin::InstanceSamplesResponse>,
    row: Row,
    interval: Option<Duration>,
}

struct Demand {
    output: mpsc::Sender<harmos_protocol::plugin::InstanceSamplesRequest>,
    input: tonic::Streaming<harmos_protocol::plugin::InstanceSamplesResponse>,
    row: Row,
    interval: Option<Duration>,
    lease: Arc<StreamWriter>,
    close: watch::Receiver<bool>,
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
        Self::activate(prepared, streams, activation, clock, operations, false)
    }
    pub(super) async fn prepare<A: Application>(
        streams: &StreamManager<A>, activation: &Activation, settings: &PlacementSettings, operations: &crate::operations::Manager,
        replace: bool,
    ) -> Result<PreparedPublications, Error> {
        let catalog = activation.description.catalog.as_ref().expect("admitted description");
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
        streams.validate_connection(&activation.entry_name, &declarations, replace).map_err(invalid)?;
        let mut demands = Vec::new();
        for (id, row, interval) in rows {
            let (output, input) = mpsc::channel(1);
            output
                .send(harmos_protocol::plugin::InstanceSamplesRequest {
                    frame: Some(harmos_protocol::plugin::instance_samples_request::Frame::Open(v1::SamplesSelection {
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
        operations: &crate::operations::Manager, replace: bool,
    ) -> Result<Self, Error> {
        let registration = streams.connect_publications(&activation.entry_name, prepared.declarations, replace).map_err(invalid)?;
        let close = watch::channel(false).0;
        let mut publications = Self {
            close,
            jobs: tokio::task::JoinSet::new(),
            registration,
        };
        for prepared in prepared.demands {
            let activity = clock.activity(format!("Publish {}", prepared.id));
            let lease = publications
                .registration
                .leases()
                .iter()
                .find(|lease| lease.stream().definition().id() == prepared.id)
                .expect("staged stream lease")
                .clone();
            let close = publications.close.subscribe();
            let connection = activation.connection.clone();
            let clock = clock.clone();
            let operations = operations.clone();
            let caller = Arc::new(super::CallingPlugin {
                name: activation.entry_name.clone(),
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
                    closed: connection.closed.subscribe(),
                    operations,
                    caller,
                };
                if let Err(error) = run(&clock, activity, demand).await {
                    tracing::warn!(%error, "plugin publication ended");
                    if !matches!(error, Error::Invalid(_)) {
                        connection.closed.send_replace(true);
                    }
                }
            });
        }
        Ok(publications)
    }

    pub(super) async fn close(&mut self) {
        self.close.send_replace(true);
        while self.jobs.join_next().await.is_some() {}
        for lease in self.registration.leases() {
            lease.revoke();
        }
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

fn interval(stream: &v1::StreamDeclaration, settings: &v1::SettingValues) -> Result<Option<Duration>, Error> {
    let mut interval = stream.interval;
    if !stream.interval_setting.is_empty() {
        let value = settings
            .values
            .get(&stream.interval_setting)
            .and_then(|value| value.value.as_ref())
            .ok_or_else(|| invalid("Read Stream Interval: setting is not declared"))?;
        let v1::setting_value::Value::Duration(value) = value else {
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

async fn run(clock: &Clock, mut activity: crate::clock::Activity, demand: Demand) -> Result<(), Error> {
    let Demand {
        output,
        input,
        row,
        interval,
        lease,
        mut close,
        closed,
        operations,
        caller,
    } = demand;
    let input = Arc::new(AsyncMutex::new(input));
    let seen = Arc::new(Mutex::new(BTreeSet::new()));
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
                    let response = activity
                        .wait(operations.call_native::<super::samples::Tick>(super::samples::Request {
                            input: input.clone(),
                            output: output.clone(),
                            caller: caller.clone(),
                            seen: seen.clone(),
                            at,
                        }))
                        .await
                        .map_err(invalid)?;
                    response
                } else {
                    let response =
                        input.lock().await.message().await.map_err(invalid)?.ok_or_else(|| invalid("Read Device Samples: demand ended"))?;
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
                let context = if interval.is_none() {
                    sample.at.map(|at| format!("{{\"source_time\":{{\"seconds\":{},\"nanos\":{}}}}}", at.seconds, at.nanos))
                } else {
                    None
                };
                let payload = row.decode(&sample.row)?;
                lease
                    .admit_clocked_row(
                        PublicationRow {
                            payload,
                            published_unix_micros: Some(published.seconds * 1_000_000 + i64::from(published.nanos) / 1000),
                            context,
                        },
                        sequence,
                        clock,
                    )
                    .map_err(invalid)?;
                lease.ingested().await.map_err(invalid)?;
            }
        };
        tokio::pin!(work);
        tokio::select! {
            biased;
            () = super::disconnected(closed) => return Ok(()),
            _ = close.changed() => {},
            result = &mut work => return result,
        }
    }
    drop(output);
    // Input EOF Ends Demand and Awaits the SDK's Producer/Observer Cleanup.
    while input.lock().await.message().await.map_err(invalid)?.is_some() {}
    Ok(())
}

/* === Tests === */
