/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Admitted Live Demand Uses the Existing Watch Cursor.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

// Third Party Imports
use futures_util::StreamExt;
use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor, ReflectMessage, Value};
use prost_types::Timestamp;

// Local Imports
use super::window::Row;
use super::{Holding, Item, ManagedPublication, StreamManager, StreamRow, Table, protobuf, window};
use harmos_domain::MessageError;
use harmos_operations::execution::operations::{ExecutionContext, ProtobufOperation, ProtobufOutput, Resource, ServingError, Visibility};
use harmos_protocol::columns::batch;
use harmos_protocol::plugin::{
    SubscriptionRow, SubscriptionsSubscribeRequest, SubscriptionsSubscribeResponse, subscriptions_subscribe_response,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Source {
    table: Arc<Table>,
    closed: Arc<AtomicBool>,
    settings: Arc<crate::config::SchemaSettings>,
}

pub(crate) struct Subscribe;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Source {
    pub(crate) fn new<A: crate::Application>(streams: &StreamManager<A>) -> Self {
        Self {
            table: streams.windows.clone(),
            closed: streams.closed.clone(),
            settings: streams.schema_settings.clone(),
        }
    }
}

impl ProtobufOperation for Subscribe {
    type Request = SubscriptionsSubscribeRequest;
    type Response = SubscriptionsSubscribeResponse;
    type Endpoints = ProtobufOutput<SubscriptionsSubscribeResponse>;
    const ID: &'static str = "harmos.plugin.v1.Subscriptions/Subscribe";
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Source>()];
    const POLICY: crate::OperationPolicy = crate::OperationPolicy {
        timeout: harmos_operations::operations::UNBOUNDED,
        attempt_timeout: harmos_operations::operations::UNBOUNDED,
        cancellation_timeout: std::time::Duration::ZERO,
        retries: crate::RetryPolicy::Never,
    };

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let source = context.resource::<Source>().map_err(invalid)?;
        let wired = source.table.current(&request.stream).ok_or_else(|| invalid("Subscribe to Stream: select a registered live stream"))?;
        let pool = DescriptorPool::decode(request.types.as_slice()).map_err(invalid)?;
        let descriptor =
            pool.get_message_by_name(&request.row).ok_or_else(|| invalid("Subscribe to Stream: supply the declared row type"))?;
        let row = protobuf::Row::new(descriptor.clone()).map_err(invalid)?;
        let schema = if wired.definition.owner().is_none() {
            #[cfg(feature = "grpc")]
            {
                let descriptor = crate::grpc::schema::descriptor_schema(wired.definition.schema(), "harmos.subscription", &source.settings)
                    .map_err(invalid)?;
                protobuf::Row::new(descriptor).map_err(invalid)?.schema
            }
            #[cfg(not(feature = "grpc"))]
            {
                return Err(invalid("Subscribe to Native Stream: enable the runtime grpc feature"));
            }
        } else {
            wired.definition.schema().clone()
        };
        let ordinary = schema.equivalent(schema.root, &row.schema, row.schema.root, &source.settings).map_err(invalid)?;
        let codec = if ordinary {
            row
        } else {
            let derived = harmos_protocol::columns::derive(&descriptor).map_err(invalid)?;
            let batch = protobuf::Row::new(derived.batch).map_err(invalid)?;
            if !schema.equivalent(schema.root, &batch.schema, batch.schema.root, &source.settings).map_err(invalid)? {
                return Err(invalid("Subscribe to Stream: refresh the row type to match the registered stream"));
            }
            batch
        };
        let held = Holding::engage(wired.window.clone());
        let mut following =
            window::projected(wired.window.clone(), source.closed.clone(), held, !ordinary, |row, observation| (row.clone(), *observation));
        let mut requested = tokio::select! {
            biased;
            _ = context.cancellation.cancelled() => return Ok(Self::Response::default()),
            result = crate::clock::waiting(output.requested_after(0)) => result.map_err(ServingError::Stream)?,
        };
        let opened = Self::Response {
            item: Some(subscriptions_subscribe_response::Item::Opened(true)),
        };
        tokio::select! {
            biased;
            _ = context.cancellation.cancelled() => return Ok(Self::Response::default()),
            result = crate::clock::waiting(output.send(opened)) => result.map_err(ServingError::Stream)?,
        }
        loop {
            requested = tokio::select! {
                biased;
                _ = context.cancellation.cancelled() => return Ok(Self::Response::default()),
                result = crate::clock::waiting(output.requested_after(requested)) => result.map_err(ServingError::Stream)?,
            };
            let item = tokio::select! {
                biased;
                _ = context.cancellation.cancelled() => return Ok(Self::Response::default()),
                item = crate::clock::waiting(following.next()) => item,
            };
            let Some(item) = item else {
                return Ok(Self::Response::default());
            };
            let Item::Row(at) = item else {
                let Item::Gap { missed } = item else { unreachable!() };
                let response = Self::Response {
                    item: Some(subscriptions_subscribe_response::Item::Missed(missed)),
                };
                tokio::select! {
                    biased;
                    _ = context.cancellation.cancelled() => return Ok(Self::Response::default()),
                    result = crate::clock::waiting(output.send(response)) => result.map_err(ServingError::Stream)?,
                }
                continue;
            };
            let (row, observation) = at.value;
            let (mut payload, source_time) = if let Some(encode) = observation.encode {
                let payload = encode(&row).map_err(invalid)?;
                (payload, observation.received)
            } else {
                let publication =
                    row.downcast_ref::<ManagedPublication>().ok_or_else(|| invalid("Subscribe to Stream: unsupported row encoding"))?;
                let payload = codec.encode(publication.payload()).map_err(invalid)?;
                let source_time = publication
                    .context()
                    .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
                    .and_then(|value| value.get("source_time").cloned())
                    .and_then(|value| {
                        Some(Timestamp {
                            seconds: value.get("seconds")?.as_i64()?,
                            nanos: value.get("nanos")?.as_i64()?.try_into().ok()?,
                        })
                    })
                    .unwrap_or(observation.received);
                (payload, source_time)
            };
            // Release Shared Publication Credit Before Waiting for This Subscriber.
            drop(row);
            let message = if ordinary {
                None
            } else {
                let message = DynamicMessage::decode(codec.descriptor.clone(), payload.as_slice()).map_err(invalid)?;
                payload = Vec::new();
                Some(message)
            };
            let count = if let Some(message) = &message {
                message
                    .get_field_by_number(batch::COUNT)
                    .and_then(|value| value.as_u64())
                    .ok_or_else(|| invalid("Read Interval Stream: invalid sample count"))?
            } else {
                1
            };
            for index in 0..count {
                if index > 0 {
                    requested = tokio::select! {
                        biased;
                        _ = context.cancellation.cancelled() => return Ok(Self::Response::default()),
                        result = crate::clock::waiting(output.requested_after(requested)) => result.map_err(ServingError::Stream)?,
                    };
                }
                let (payload, source_time) = if let Some(message) = &message {
                    let (row, source_time) = sample(message, descriptor.clone(), index).map_err(invalid)?;
                    (row.encode_to_vec(), source_time)
                } else {
                    (std::mem::take(&mut payload), source_time)
                };
                let response = Self::Response {
                    item: Some(subscriptions_subscribe_response::Item::Row(SubscriptionRow {
                        payload,
                        source: Some(source_time),
                        received: Some(observation.received),
                    })),
                };
                tokio::select! {
                    biased;
                    _ = context.cancellation.cancelled() => return Ok(Self::Response::default()),
                    result = crate::clock::waiting(output.send(response)) => result.map_err(ServingError::Stream)?,
                }
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn encode_native<S: StreamRow>(row: &Row) -> Result<Vec<u8>, MessageError> {
    let value = row.downcast_ref::<S>().expect("a native window holds its declared row");
    let schema = harmos_domain::schema::graph::Schema::from_declaration(S::DEFINITION, &Default::default())
        .map_err(|error| MessageError::new(error.to_string()))?;
    let codec = harmos_domain::schema::json::JsonCodec::new(
        schema.root().map_err(|error| MessageError::new(error.to_string()))?,
        &Default::default(),
    )?;
    let value = serde_json::to_value(value).map_err(|error| MessageError::new(error.to_string()))?;
    let canonical = codec.encode(&value)?;
    let mut payload = Vec::new();
    harmos_operations::execution::grpc::encode_postcard(&S::DEFINITION, &canonical, &mut payload)?;
    Ok(payload)
}

fn sample(batch: &DynamicMessage, descriptor: MessageDescriptor, index: u64) -> Result<(DynamicMessage, Timestamp), String> {
    let start = batch
        .get_field_by_number(batch::START_UNIX_NANOS)
        .and_then(|value| value.as_i64())
        .ok_or("Read Interval Stream: missing source grid")?;
    let period = batch
        .get_field_by_number(batch::PERIOD_NANOS)
        .and_then(|value| value.as_u64())
        .ok_or("Read Interval Stream: missing source period")?;
    let columns = batch.get_field_by_number(batch::COLUMNS).ok_or("Read Interval Stream: missing columns")?;
    let Value::Message(columns) = columns.as_ref() else {
        return Err("Read Interval Stream: invalid columns".into());
    };
    let mut row = DynamicMessage::new(descriptor.clone());
    for field in descriptor.fields() {
        let values = columns.get_field_by_number(field.number()).ok_or("Read Interval Stream: missing column")?;
        let Value::List(values) = values.as_ref() else {
            return Err("Read Interval Stream: invalid column".into());
        };
        let mut value =
            values.get(usize::try_from(index).map_err(|error| error.to_string())?).ok_or("Read Interval Stream: column too short")?.clone();
        if field.field_descriptor_proto().proto3_optional.unwrap_or(false) {
            let Value::Message(element) = value else {
                return Err("Read Interval Stream: invalid optional column".into());
            };
            let presence = element.descriptor().get_field(1).ok_or("Read Interval Stream: missing presence")?;
            if !element.has_field(&presence) {
                continue;
            }
            value = element.get_field(&presence).into_owned();
        }
        row.set_field(&field, value);
    }
    let nanos = i128::from(start) + i128::from(period) * i128::from(index);
    let seconds = i64::try_from(nanos.div_euclid(1_000_000_000)).map_err(|error| error.to_string())?;
    Ok((
        row,
        Timestamp {
            seconds,
            nanos: nanos.rem_euclid(1_000_000_000) as i32,
        },
    ))
}

fn invalid(error: impl std::fmt::Display) -> ServingError {
    ServingError::Handler(harmos_operations::Failure::from(harmos_protocol::model::Failure {
        classification: harmos_protocol::model::Classification::Refused as i32,
        code: "stream_contract".into(),
        message: error.to_string(),
        action: "Check the Stream Declaration and Row Type".into(),
        operation: Subscribe::ID.into(),
        plugin: "host".into(),
        ..Default::default()
    }))
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streams::StreamEntry;
    use crate::{Application, Lifecycle, Runtime, Setup, Startup};
    use harmos_protocol::host::TimeGetResponse;
    use prost_types::field_descriptor_proto::{Label, Type};
    use prost_types::{DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet, OneofDescriptorProto};
    use std::num::NonZeroUsize;
    use std::time::{Duration, SystemTime};

    struct Model;

    #[derive(crate::Row, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    #[harmos_path(crate)]
    struct Native {
        value: u64,
    }

    impl StreamRow for Native {
        const ENTRY: StreamEntry<Self> = StreamEntry::new("fixture.samples", 8);
    }

    impl Lifecycle for Model {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = Startup<Self>;

        async fn prepare(_: (), _: &mut Setup<Self>) -> Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    impl Application for Model {
        type State = ();
        type Transaction = crate::state::Never;
        type Record = ();
    }

    #[tokio::test]
    async fn independent_windows_report_the_exact_gap_without_prefetching_stalled_rows() {
        let (clock, controller) = crate::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH + Duration::from_secs(11));
        let runtime = Runtime::<Model>::start_with((), (), |setup| {
            setup.settings().clock(clock).register([Native::definition()]);
            setup.settings().operation_settings(crate::operations::Settings {
                preparation_bytes: NonZeroUsize::new(32 * 1024 * 1024),
                ..Default::default()
            });
        })
        .await
        .unwrap();
        let schema = Native::definition();
        let descriptor = crate::grpc::schema::descriptor_schema(schema.schema(), "fixture", &Default::default()).unwrap();
        let request = SubscriptionsSubscribeRequest {
            stream: Native::ID.into(),
            row: descriptor.full_name().into(),
            types: prost_types::FileDescriptorSet {
                file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
            }
            .encode_to_vec(),
        };
        let limits = crate::operations::StreamSettings {
            items: NonZeroUsize::MIN,
            bytes: NonZeroUsize::new(4096).unwrap(),
        };
        let mut fast = runtime.operations.download_protobuf::<Subscribe>(request.clone(), limits).unwrap();
        let mut slow = runtime.operations.download_protobuf::<Subscribe>(request, limits).unwrap();
        assert!(matches!(fast.next().await.unwrap().unwrap().item, Some(subscriptions_subscribe_response::Item::Opened(true))));
        assert!(matches!(slow.next().await.unwrap().unwrap().item, Some(subscriptions_subscribe_response::Item::Opened(true))));
        let demand = runtime.streams.demand::<Native>();
        assert_eq!(*demand.borrow(), 2);
        let published = Native::WINDOW as u64 * 3;
        for value in 0..published {
            runtime.streams.publish(Native { value });
            let response = fast.next().await.unwrap().unwrap();
            let Some(subscriptions_subscribe_response::Item::Row(row)) = response.item else {
                panic!("Expected Native Row");
            };
            let row_value = DynamicMessage::decode(descriptor.clone(), row.payload.as_slice()).unwrap();
            assert_eq!(row_value.get_field_by_name("value").unwrap().as_u64(), Some(value));
            assert_eq!(row.source, Some(Timestamp { seconds: 11, nanos: 0 }));
            assert_eq!(row.received, row.source);
        }
        let missed = published - Native::WINDOW as u64;
        assert!(
            matches!(slow.next().await.unwrap().unwrap().item, Some(subscriptions_subscribe_response::Item::Missed(count)) if count == missed)
        );
        for value in missed..published {
            let response = slow.next().await.unwrap().unwrap();
            let Some(subscriptions_subscribe_response::Item::Row(row)) = response.item else {
                panic!("Expected Retained Row");
            };
            assert_eq!(
                DynamicMessage::decode(descriptor.clone(), row.payload.as_slice()).unwrap().get_field_by_name("value").unwrap().as_u64(),
                Some(value)
            );
        }
        slow.cancel_and_settle().await.unwrap();
        assert_eq!(*demand.borrow(), 1);
        controller.advance_by(Duration::from_nanos(123456789)).await;
        let received: Timestamp = controller.read().date.into();
        runtime.streams.publish(Native { value: published });
        let response = fast.next().await.unwrap().unwrap();
        let Some(subscriptions_subscribe_response::Item::Row(row)) = response.item else {
            panic!("Expected Continuing Row");
        };
        assert_eq!(row.received, Some(received));
        fast.request_cancel();
        fast.cancel_and_settle().await.unwrap();
        assert_eq!(*demand.borrow(), 0);
        runtime.stop().await.unwrap();
    }

    #[test]
    fn interval_rows_keep_the_source_grid_through_canonical_storage() {
        let descriptor = TimeGetResponse::default().descriptor();
        let derived = harmos_protocol::columns::derive(&descriptor).unwrap();
        let mut columns = DynamicMessage::new(derived.columns);
        let at = Timestamp { seconds: 3, nanos: 42 };
        let value = DynamicMessage::decode(at.descriptor(), at.encode_to_vec().as_slice()).unwrap();
        columns.set_field_by_number(1, Value::List(vec![Value::Message(value.clone()), Value::Message(value)]));
        let mut message = DynamicMessage::new(derived.batch.clone());
        message.set_field_by_number(batch::START_UNIX_NANOS, Value::I64(7_123_456_789));
        message.set_field_by_number(batch::PERIOD_NANOS, Value::U64(20));
        message.set_field_by_number(batch::COUNT, Value::U64(2));
        message.set_field_by_number(batch::COLUMNS, Value::Message(columns));
        let codec = protobuf::Row::new(derived.batch).unwrap();
        let canonical = codec.decode(&message.encode_to_vec()).unwrap();
        let payload = codec.encode(&canonical).unwrap();
        let stored = DynamicMessage::decode(codec.descriptor.clone(), payload.as_slice()).unwrap();
        for index in 0..2 {
            let (row, source) = sample(&stored, descriptor.clone(), index).unwrap();
            assert_eq!(TimeGetResponse::decode(row.encode_to_vec().as_slice()).unwrap().now, Some(at));
            assert_eq!(
                source,
                Timestamp {
                    seconds: 7,
                    nanos: 123456789 + index as i32 * 20
                }
            );
        }
    }

    #[test]
    fn optional_interval_values_keep_absent_zero_and_nonzero_presence() {
        let pool = DescriptorPool::from_file_descriptor_set(FileDescriptorSet {
            file: vec![FileDescriptorProto {
                name: Some("optional.proto".into()),
                package: Some("fixture".into()),
                syntax: Some("proto3".into()),
                message_type: vec![DescriptorProto {
                    name: Some("Optional".into()),
                    field: vec![FieldDescriptorProto {
                        name: Some("value".into()),
                        number: Some(1),
                        r#type: Some(Type::Double as i32),
                        label: Some(Label::Optional as i32),
                        proto3_optional: Some(true),
                        oneof_index: Some(0),
                        ..Default::default()
                    }],
                    oneof_decl: vec![OneofDescriptorProto {
                        name: Some("_value".into()),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        })
        .unwrap();
        let descriptor = pool.get_message_by_name("fixture.Optional").unwrap();
        let derived = harmos_protocol::columns::derive(&descriptor).unwrap();
        let prost_reflect::Kind::Message(element) = derived.columns.get_field(1).unwrap().kind() else { unreachable!() };
        let values = [None, Some(0.0), Some(2.5)];
        let elements = values
            .iter()
            .map(|value| {
                let mut message = DynamicMessage::new(element.clone());
                if let Some(value) = value {
                    message.set_field_by_number(1, Value::F64(*value));
                }
                Value::Message(message)
            })
            .collect();
        let mut columns = DynamicMessage::new(derived.columns);
        columns.set_field_by_number(1, Value::List(elements));
        let mut message = DynamicMessage::new(derived.batch.clone());
        message.set_field_by_number(batch::START_UNIX_NANOS, Value::I64(7_000_000_000));
        message.set_field_by_number(batch::PERIOD_NANOS, Value::U64(20));
        message.set_field_by_number(batch::COUNT, Value::U64(values.len() as u64));
        message.set_field_by_number(batch::COLUMNS, Value::Message(columns));
        let codec = protobuf::Row::new(derived.batch).unwrap();
        let rows = codec.decode_batch(&message.encode_to_vec(), values.len() as u64, 20).unwrap();
        let payload = codec.encode(&rows[0].2).unwrap();
        let stored = DynamicMessage::decode(codec.descriptor.clone(), payload.as_slice()).unwrap();
        let field = descriptor.get_field(1).unwrap();
        for (index, value) in values.iter().enumerate() {
            let (row, _) = sample(&stored, descriptor.clone(), index as u64).unwrap();
            assert_eq!(row.has_field(&field), value.is_some());
            if let Some(value) = value {
                assert_eq!(row.get_field(&field).as_f64(), Some(*value));
            }
        }
    }

    #[tokio::test]
    async fn device_timestamps_and_interval_overruns_retain_source_sample_counts() {
        let (clock, _) = crate::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH + Duration::from_secs(11));
        let runtime = Runtime::<Model>::start_with((), (), |setup| {
            setup.settings().clock(clock).operation_settings(crate::operations::Settings {
                preparation_bytes: NonZeroUsize::new(32 * 1024 * 1024),
                ..Default::default()
            });
        })
        .await
        .unwrap();
        let descriptor = TimeGetResponse::default().descriptor();
        let codec = protobuf::Row::new(descriptor.clone()).unwrap();
        let derived = harmos_protocol::columns::derive(&descriptor).unwrap();
        let batch_codec = protobuf::Row::new(derived.batch.clone()).unwrap();
        let declarations =
            [("fixture.device", "Device", codec.schema.clone()), ("fixture.interval", "Interval", batch_codec.schema.clone())].map(
                |(id, label, row)| harmos_operations::StreamDeclaration {
                    id: id.into(),
                    window: Native::WINDOW,
                    row,
                    row_metadata: vec![],
                    metadata: harmos_domain::Metadata::new(harmos_domain::Identifier::new(id).unwrap(), label, None),
                },
            );
        let connection = runtime
            .streams
            .register_plugin(
                "fixture",
                1,
                declarations,
                false,
                #[cfg(feature = "plugin-guest")]
                None,
            )
            .unwrap();
        let types = prost_types::FileDescriptorSet {
            file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        }
        .encode_to_vec();
        let request = |stream: &str| SubscriptionsSubscribeRequest {
            stream: stream.into(),
            row: descriptor.full_name().into(),
            types: types.clone(),
        };
        let limits = crate::operations::StreamSettings {
            items: NonZeroUsize::MIN,
            bytes: NonZeroUsize::new(4096).unwrap(),
        };
        let mut device = runtime.operations.download_protobuf::<Subscribe>(request("fixture.device"), limits).unwrap();
        device.next().await.unwrap().unwrap();
        let source = Timestamp { seconds: 3, nanos: 123456789 };
        let value = TimeGetResponse { now: Some(source) };
        let writer = &connection.leases()[0];
        assert_eq!(writer.stream().definition().id(), "fixture.device");
        writer
            .publish_row(
                super::super::PublicationRow {
                    payload: codec.decode(&value.encode_to_vec()).unwrap(),
                    published_unix_micros: Some(11_000_000),
                    context: Some(serde_json::json!({"source_time": {"seconds": source.seconds, "nanos": source.nanos}}).to_string()),
                },
                0,
            )
            .unwrap();
        let response = device.next().await.unwrap().unwrap();
        let Some(subscriptions_subscribe_response::Item::Row(row)) = response.item else {
            panic!("Expected Device Row");
        };
        assert_eq!(row.source, Some(source));
        assert_eq!(row.received, Some(Timestamp { seconds: 11, nanos: 0 }));
        assert_eq!(TimeGetResponse::decode(row.payload.as_slice()).unwrap().now, Some(source));
        device.cancel_and_settle().await.unwrap();

        let mut fast = runtime.operations.download_protobuf::<Subscribe>(request("fixture.interval"), limits).unwrap();
        let mut slow = runtime.operations.download_protobuf::<Subscribe>(request("fixture.interval"), limits).unwrap();
        fast.next().await.unwrap().unwrap();
        slow.next().await.unwrap().unwrap();
        let writer = &connection.leases()[1];
        let parts = Native::WINDOW as u64 * 3;
        for part in 0..parts {
            let mut columns = DynamicMessage::new(derived.columns.clone());
            let value = DynamicMessage::decode(source.descriptor(), source.encode_to_vec().as_slice()).unwrap();
            columns.set_field_by_number(1, Value::List(vec![Value::Message(value.clone()), Value::Message(value)]));
            let mut stored = DynamicMessage::new(derived.batch.clone());
            stored.set_field_by_number(batch::START_UNIX_NANOS, Value::I64(7_000_000_000 + part as i64 * 40));
            stored.set_field_by_number(batch::PERIOD_NANOS, Value::U64(20));
            stored.set_field_by_number(batch::COUNT, Value::U64(2));
            stored.set_field_by_number(batch::COLUMNS, Value::Message(columns));
            writer
                .publish_row(
                    super::super::PublicationRow {
                        payload: batch_codec.decode(&stored.encode_to_vec()).unwrap(),
                        published_unix_micros: None,
                        context: None,
                    },
                    0,
                )
                .unwrap();
            writer.ingested().await.unwrap();
            for sample in 0..2 {
                let response = fast.next().await.unwrap().unwrap();
                let Some(subscriptions_subscribe_response::Item::Row(row)) = response.item else {
                    panic!("Expected Interval Row");
                };
                assert_eq!(
                    row.source,
                    Some(Timestamp {
                        seconds: 7,
                        nanos: part as i32 * 40 + sample * 20
                    })
                );
                assert_eq!(row.received, Some(Timestamp { seconds: 11, nanos: 0 }));
            }
        }
        let missed = (parts - Native::WINDOW as u64) * 2;
        assert!(
            matches!(slow.next().await.unwrap().unwrap().item, Some(subscriptions_subscribe_response::Item::Missed(count)) if count == missed)
        );
        let response = slow.next().await.unwrap().unwrap();
        let Some(subscriptions_subscribe_response::Item::Row(row)) = response.item else {
            panic!("Expected Oldest Retained Sample");
        };
        assert_eq!(row.source, Some(Timestamp { seconds: 7, nanos: missed as i32 * 20 }));
        slow.cancel_and_settle().await.unwrap();
        fast.cancel_and_settle().await.unwrap();
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn deferred_publication_keeps_its_original_receipt_time() {
        let (clock, controller) = crate::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH + Duration::from_secs(11));
        let codec = protobuf::Row::new(TimeGetResponse::default().descriptor()).unwrap();
        let definition =
            super::super::Definition::declared::<super::super::PublicationRow>("fixture.device", codec.schema.clone(), Native::WINDOW)
                .unwrap()
                .publication("fixture");
        let pool = super::super::publication::Pool::new(clock.clone());
        let publication = pool
            .retain(
                &definition,
                None,
                super::super::PublicationRow {
                    payload: codec.decode(&TimeGetResponse::default().encode_to_vec()).unwrap(),
                    published_unix_micros: None,
                    context: None,
                },
                1,
                super::super::Missed::Rows(0),
                None,
            )
            .unwrap();
        let window = Arc::new(window::Window::wired(
            Native::WINDOW,
            crate::state::Position::ORIGIN,
            Arc::new(std::sync::atomic::AtomicU64::new(0)),
            tokio::sync::watch::channel(()).0,
            clock,
        ));
        let held = Holding::engage(window.clone());
        let mut following =
            window::projected(window.clone(), Arc::new(AtomicBool::new(false)), held, true, |_, observation| observation.received);
        controller.advance_by(Duration::from_secs(1)).await;
        window.append_locked(crate::state::Position::ORIGIN, Arc::new(publication), 0).unwrap();
        let item = following.next().await.unwrap();
        assert!(matches!(item, Item::Row(at) if at.value == Timestamp { seconds: 11, nanos: 0 }));
    }
}
