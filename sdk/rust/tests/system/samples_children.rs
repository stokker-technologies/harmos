/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Samples Child Call Journeys

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::HashMap;

// Third Party Imports
use prost::Message;
use prost_types::Timestamp;

// Local Imports
use super::support;
use harmos::InstanceState;
use harmos_protocol::host::plugins::SettingsListResponse;
use harmos_protocol::host::{InvocationsWatchResponse, TimeGetResponse};
use harmos_protocol::model::Classification;
use harmos_protocol::model::Failure;
use harmos_protocol::model::SettingValue;
use harmos_protocol::model::SettingValues;
use harmos_protocol::model::setting_value;
use harmos_protocol::plugin::ChildResponse;
use harmos_protocol::plugin::Window;
use harmos_protocol::plugin::child_response;
use harmos_protocol::plugin::{InstanceSamplesRequest, instance_samples_request, instance_samples_response};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// The Columns of the Fixture's Reading Row: One Value per Sample of a Window.
#[derive(prost::Message)]
struct Readings {
    #[prost(uint64, repeated, tag = "1")]
    sequence: Vec<u64>,
    #[prost(int64, repeated, tag = "2")]
    seconds: Vec<i64>,
    #[prost(int64, repeated, tag = "3")]
    setting: Vec<i64>,
    #[prost(bytes = "vec", repeated, tag = "4")]
    failure: Vec<Vec<u8>>,
}

#[derive(prost::Message)]
struct Isolation {
    #[prost(bool, tag = "1")]
    absent: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn samples_children_preserve_correlation_failures_watch_and_window_lifetimes() {
    let scratch = tempfile::tempdir().unwrap();
    let cleanup = scratch.path().join("cleanup");
    let mut process = support::Process::start(&cleanup).await.unwrap();
    let (send, mut output) = support::publish(&mut process.client, "execution.children").await.unwrap();
    let expected_failure = Failure {
        classification: Classification::Refused as i32,
        code: "clock_refused".into(),
        operation: "clock".into(),
        plugin: "host".into(),
        diagnostics: "clock evidence".into(),
        details: HashMap::from([("key".into(), "value".into())]),
        cause: Some(Box::new(Failure {
            code: "source".into(),
            ..Default::default()
        })),
        ..Default::default()
    };
    let mut last_call = 0;
    for sequence in 1..=2 {
        // Reply Out of Order Without Advancing a Wall Clock.
        let at = Timestamp { seconds: sequence as i64, nanos: 250 };
        send.send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Window(Window {
                start: Some(at),
                period: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
                count: 1,
                sequence,
                capturing_executions: Vec::new(),
            })),
        })
        .await
        .unwrap();
        let first = output.message().await.unwrap().unwrap();
        let second = output.message().await.unwrap().unwrap();
        assert!(first.samples.is_empty() && second.samples.is_empty());
        let Some(instance_samples_response::Frame::Child(first)) = first.frame else {
            panic!("expected first child")
        };
        let Some(instance_samples_response::Frame::Child(second)) = second.frame else {
            panic!("expected second child")
        };
        assert!(first.call > last_call && second.call > first.call);
        assert_eq!(first.operation, "harmos.plugin.v1.Time/Get");
        assert_eq!(second.operation, "harmos.plugin.v1.plugins.Settings/List");
        let settings = SettingsListResponse {
            value: Some(SettingValues {
                values: HashMap::from([(
                    "threshold".into(),
                    SettingValue {
                        value: Some(setting_value::Value::Integer(sequence as i64)),
                    },
                )]),
            }),
            ..Default::default()
        };
        send.send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Child(ChildResponse {
                call: second.call,
                result: Some(child_response::Result::Payload(settings.encode_to_vec())),
            })),
        })
        .await
        .unwrap();
        let result = if sequence == 1 {
            child_response::Result::Failure(expected_failure.clone())
        } else {
            child_response::Result::Payload(TimeGetResponse { now: Some(at) }.encode_to_vec())
        };
        send.send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Child(ChildResponse { call: first.call, result: Some(result) })),
        })
        .await
        .unwrap();
        // Demand a Watch Item, Then Let the Window Answer Cancel the Open Watch.
        let Some(instance_samples_response::Frame::Child(watch)) = output.message().await.unwrap().unwrap().frame else {
            panic!("expected watch")
        };
        assert!(watch.watch && watch.call > second.call);
        let Some(instance_samples_response::Frame::ChildDemand(demand)) = output.message().await.unwrap().unwrap().frame else {
            panic!("expected demand")
        };
        assert_eq!(demand.call, watch.call);
        send.send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Child(ChildResponse {
                call: watch.call,
                result: Some(child_response::Result::Item(
                    InvocationsWatchResponse {
                        invocation: Some(InstanceState {
                            phase: "running".into(),
                            ..Default::default()
                        }),
                    }
                    .encode_to_vec(),
                )),
            })),
        })
        .await
        .unwrap();
        let Some(instance_samples_response::Frame::ChildCancel(cancel)) = output.message().await.unwrap().unwrap().frame else {
            panic!("expected cancellation")
        };
        assert_eq!(cancel.call, watch.call);
        let answer = output.message().await.unwrap().unwrap();
        assert!(answer.samples.is_empty());
        let Some(instance_samples_response::Frame::Batch(batch)) = answer.frame else {
            panic!("expected the window's batch")
        };
        assert_eq!((batch.offset, batch.count, batch.last), (0, 1, true));
        let row = Readings::decode(batch.columns.as_slice()).unwrap();
        assert_eq!(row.sequence, [sequence]);
        assert_eq!(row.setting, [sequence as i64]);
        assert_eq!(
            row.seconds,
            [if sequence == 1 {
                -1
            } else {
                sequence as i64
            }]
        );
        last_call = watch.call;
        if sequence == 1 {
            assert_eq!(Failure::decode(row.failure[0].as_slice()).unwrap(), expected_failure);
        } else {
            assert!(row.failure[0].is_empty());
        }
        let (_send, mut result) = support::invoke(&mut process.client, "execution.stale").await.unwrap();
        let result = support::terminal::<Isolation>(&mut result).await.unwrap();
        assert!(result.absent);
    }
    drop(send);
    assert!(output.message().await.unwrap().is_none());

    // Refuse Child Traffic Outside a Window, Including Device-Originated Streams.
    for name in ["execution.children", "execution.device"] {
        let (send, mut output) = support::publish(&mut process.client, name).await.unwrap();
        send.send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Child(ChildResponse {
                call: 1,
                result: Some(child_response::Result::Payload(Vec::new())),
            })),
        })
        .await
        .unwrap();
        assert_eq!(output.message().await.unwrap_err().code(), tonic::Code::InvalidArgument);
    }
    process.client.stop(harmos_protocol::plugin::InstanceStopRequest { reason: String::new() }).await.unwrap();
    process.close().await.unwrap();
}
