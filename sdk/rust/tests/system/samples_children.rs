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

// Third Party Imports

// Local Imports
use super::support;
use harmos::{InstanceState, Message};
use harmos_protocol::plugin::host::{InvocationsWatchResponse, SettingsListResponse, TimeGetResponse};
use harmos_protocol::plugin::{InstanceSamplesRequest, instance_samples_request, instance_samples_response};
use harmos_protocol::v1;
use prost::Message as _;
use prost_types::Timestamp;
use std::collections::HashMap;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Message)]
struct Reading {
    sequence: u64,
    seconds: i64,
    setting: i64,
    failure: Vec<u8>,
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
async fn samples_children_preserve_correlation_failures_watch_and_tick_lifetimes() {
    let scratch = tempfile::tempdir().unwrap();
    let cleanup = scratch.path().join("cleanup");
    let mut process = support::Process::start(&cleanup).await.unwrap();
    let (send, mut output) = support::publish(&mut process.client, "execution.children").await.unwrap();
    let expected_failure = v1::Failure {
        classification: v1::Classification::Refused as i32,
        code: "clock_refused".into(),
        operation: "clock".into(),
        plugin: "host".into(),
        diagnostics: "clock evidence".into(),
        details: HashMap::from([("key".into(), "value".into())]),
        cause: Some(Box::new(v1::Failure {
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
            frame: Some(instance_samples_request::Frame::Tick(v1::Tick { at: Some(at) })),
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
        assert_eq!(first.operation, "harmos.plugin.host.v1.Time/Get");
        assert_eq!(second.operation, "harmos.plugin.host.v1.Settings/List");
        let settings = SettingsListResponse {
            value: Some(v1::SettingValues {
                values: HashMap::from([(
                    "threshold".into(),
                    v1::SettingValue {
                        value: Some(v1::setting_value::Value::Integer(sequence as i64)),
                    },
                )]),
            }),
            ..Default::default()
        };
        send.send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Child(v1::ChildResponse {
                call: second.call,
                result: Some(v1::child_response::Result::Payload(settings.encode_to_vec())),
            })),
        })
        .await
        .unwrap();
        let result = if sequence == 1 {
            v1::child_response::Result::Failure(expected_failure.clone())
        } else {
            v1::child_response::Result::Payload(TimeGetResponse { now: Some(at) }.encode_to_vec())
        };
        send.send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Child(v1::ChildResponse { call: first.call, result: Some(result) })),
        })
        .await
        .unwrap();
        // Demand a Watch Item, Then Let the Tick Answer Cancel the Open Watch.
        let Some(instance_samples_response::Frame::Child(watch)) = output.message().await.unwrap().unwrap().frame else {
            panic!("expected watch")
        };
        assert!(watch.watch && watch.call > second.call);
        let Some(instance_samples_response::Frame::ChildDemand(demand)) = output.message().await.unwrap().unwrap().frame else {
            panic!("expected demand")
        };
        assert_eq!(demand.call, watch.call);
        send.send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Child(v1::ChildResponse {
                call: watch.call,
                result: Some(v1::child_response::Result::Item(
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
        assert!(answer.frame.is_none());
        assert_eq!(answer.samples.len(), 1);
        assert_eq!(answer.samples[0].at, Some(at));
        assert_eq!(answer.samples[0].sequence, sequence);
        let row = Reading::decode(answer.samples[0].row.as_slice()).unwrap();
        assert_eq!(row.sequence, sequence);
        assert_eq!(row.setting, sequence as i64);
        assert_eq!(
            row.seconds,
            if sequence == 1 {
                -1
            } else {
                sequence as i64
            }
        );
        last_call = watch.call;
        if sequence == 1 {
            assert_eq!(v1::Failure::decode(row.failure.as_slice()).unwrap(), expected_failure);
        } else {
            assert!(row.failure.is_empty());
        }
        let (_send, mut result) = support::invoke(&mut process.client, "execution.stale").await.unwrap();
        let result = support::terminal::<Isolation>(&mut result).await.unwrap();
        assert!(result.absent);
    }
    drop(send);
    assert!(output.message().await.unwrap().is_none());

    // Refuse Child Traffic Outside a Tick, Including Device-Originated Streams.
    for name in ["execution.children", "execution.device"] {
        let (send, mut output) = support::publish(&mut process.client, name).await.unwrap();
        send.send(InstanceSamplesRequest {
            frame: Some(instance_samples_request::Frame::Child(v1::ChildResponse {
                call: 1,
                result: Some(v1::child_response::Result::Payload(Vec::new())),
            })),
        })
        .await
        .unwrap();
        assert_eq!(output.message().await.unwrap_err().code(), tonic::Code::InvalidArgument);
    }
    process.client.stop(harmos_protocol::plugin::InstanceStopRequest { reason: String::new() }).await.unwrap();
    process.close().await.unwrap();
}
