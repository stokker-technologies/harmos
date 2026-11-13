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
use std::{sync::Arc, time::Duration};

// Third Party Imports
use futures_util::stream;
use tokio::sync::{mpsc, watch};

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

struct Demand {
    output: mpsc::Sender<harmos_protocol::plugin::InstanceSamplesRequest>,
    input: tonic::Streaming<harmos_protocol::plugin::InstanceSamplesResponse>,
    row: Row,
    interval: Option<Duration>,
    lease: Arc<StreamWriter>,
    close: watch::Receiver<bool>,
    closed: watch::Receiver<bool>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Publications {
    pub(super) async fn open<A: Application>(
        streams: &StreamManager<A>, activation: &Activation, clock: &Clock, settings: &PlacementSettings,
    ) -> Result<Self, Error> {
        let catalog = activation.description.catalog.as_ref().unwrap();
        let pool = prost_reflect::DescriptorPool::from_file_descriptor_set(catalog.types.clone().unwrap_or_default()).map_err(invalid)?;
        let mut declarations = Vec::new();
        let mut rows = Vec::new();
        for stream in &catalog.streams {
            let descriptor = pool.get_message_by_name(stream.row.trim_start_matches('.')).ok_or_else(|| invalid("Missing Stream Row"))?;
            let row = Row::new(descriptor)?;
            let interval = interval(stream, catalog, &pool)?;
            declarations.push(harmos_operations::StreamDeclaration {
                id: stream.id.clone(),
                window: settings.stream_window.get(),
                row: row.schema.clone(),
                metadata: crate::Metadata::new(crate::Identifier::new(&stream.id).map_err(invalid)?, stream.id.clone(), None),
                row_metadata: Vec::new(),
            });
            rows.push((stream, row, interval));
        }
        let registration = streams.connect_publications(&activation.description.id, declarations).map_err(invalid)?;
        let close = watch::channel(false).0;
        let mut publications = Self {
            close,
            jobs: tokio::task::JoinSet::new(),
            registration,
        };
        for (stream, row, interval) in rows {
            let activity = clock.activity(format!("Publish {}", stream.id));
            let (output, input) = mpsc::channel(1);
            output
                .send(harmos_protocol::plugin::InstanceSamplesRequest {
                    frame: Some(harmos_protocol::plugin::instance_samples_request::Frame::Open(v1::SamplesSelection {
                        stream: stream.id.clone(),
                        scope: String::new(),
                    })),
                })
                .await
                .unwrap();
            let input = stream::unfold(input, |mut input| async { input.recv().await.map(|frame| (frame, input)) });
            let mut client = activation.connection.client();
            let input = client.samples(input).await.map_err(|error| Error::Transport(error.to_string()))?.into_inner();
            let lease =
                publications.registration.leases().iter().find(|lease| lease.stream().definition().id() == stream.id).unwrap().clone();
            let close = publications.close.subscribe();
            let connection = activation.connection.clone();
            let clock = clock.clone();
            publications.jobs.spawn(async move {
                let demand = Demand {
                    output,
                    input,
                    row,
                    interval,
                    lease,
                    close,
                    closed: connection.closed.subscribe(),
                };
                let result = run(&clock, activity, demand).await;
                if let Err(error) = result {
                    tracing::warn!(%error, "plugin publication failed; closing its connection");
                    connection.closed.send_replace(true);
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

fn interval(
    stream: &v1::StreamDeclaration, catalog: &v1::Catalog, pool: &prost_reflect::DescriptorPool,
) -> Result<Option<Duration>, Error> {
    use prost::Message;
    let mut interval = stream.interval;
    if !stream.interval_setting.is_empty() {
        let (section_type, name) =
            stream.interval_setting.rsplit_once('.').ok_or_else(|| invalid("Interval Setting Must Name a Section Field"))?;
        let section = catalog
            .sections
            .iter()
            .find(|section| section.r#type.trim_start_matches('.') == section_type.trim_start_matches('.'))
            .ok_or_else(|| invalid("Missing Interval Setting Section"))?;
        let descriptor =
            pool.get_message_by_name(section_type.trim_start_matches('.')).ok_or_else(|| invalid("Missing Interval Setting Type"))?;
        let message = prost_reflect::DynamicMessage::decode(descriptor.clone(), section.payload.as_slice()).map_err(invalid)?;
        let field = descriptor.get_field_by_name(name).ok_or_else(|| invalid("Missing Interval Setting Field"))?;
        let value = message.get_field(&field);
        let prost_reflect::Value::Message(value) = value.as_ref() else {
            return Err(invalid("Interval Setting Must Be a Duration"));
        };
        if !matches!(field.kind(), prost_reflect::Kind::Message(kind) if kind.full_name() == "google.protobuf.Duration") {
            return Err(invalid("Interval Setting Must Be a Duration"));
        }
        interval = Some(prost_types::Duration::decode(value.encode_to_vec().as_slice()).map_err(invalid)?);
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
        mut input,
        row,
        interval,
        lease,
        mut close,
        closed,
    } = demand;
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
                            frame = input.message() => Err(invalid(format!("Unexpected Interval InstanceSamplesResponse: {frame:?}"))),
                            () = next => Ok(()),
                        }
                    };
                    activity.wait(wait).await?;
                    let reading = clock.read();
                    next_tick = Some(reading.instant + period);
                    let at = reading.date.into();
                    output
                        .send(harmos_protocol::plugin::InstanceSamplesRequest {
                            frame: Some(harmos_protocol::plugin::instance_samples_request::Frame::Tick(v1::Tick { at: Some(at) })),
                        })
                        .await
                        .map_err(invalid)?;
                    Some(at)
                } else {
                    None
                };
                let samples = input.message().await.map_err(invalid)?.ok_or_else(|| invalid("Publish Ended Before Demand Closed"))?;
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
    while input.message().await.map_err(invalid)?.is_some() {}
    Ok(())
}

/* === Tests === */
