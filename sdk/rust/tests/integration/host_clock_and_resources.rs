/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Host Clock and Execution Resources

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::error;
use std::{fs, time::Duration};

// Third Party Imports
use prost::Message;
use prost_types::Timestamp;

// Local Imports
use super::support;
use harmos_protocol::host::{TimeGetResponse, TimeSleepRequest, TimeSleepResponse};
use harmos_protocol::plugin::Window;
use harmos_protocol::plugin::{InstanceInvokeRequest, InstanceSamplesRequest, InstanceStopRequest, instance_samples_request};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(prost::Message)]
struct Evidence {
    #[prost(uint64, tag = "1")]
    resource: u64,

    #[prost(message, optional, tag = "2")]
    at: Option<Timestamp>,
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
async fn host_clock_and_resources() -> Result<(), Box<dyn error::Error>> {
    let story = async {
        // Start the Remote SDK Plugin and Admit Its Clock Requirements.
        let scratch = tempfile::tempdir()?;
        let cleanup = scratch.path().join("cleanup");
        let mut process = support::Process::start(&cleanup).await?;
        let first = support::describe(&mut process.client).await?;
        let declarations = first.declarations.as_ref().ok_or("Missing Declarations")?;
        assert_eq!(
            declarations.requires.iter().map(|requirement| requirement.id.as_str()).collect::<Vec<_>>(),
            [
                "harmos.plugin.v1.plugins.Settings/List",
                "harmos.plugin.v1.Invocations/Watch",
                "harmos.plugin.v1.Time/Sleep",
                "harmos.plugin.v1.Time/Get"
            ]
        );

        // Begin an Execution That Stores a Resource and Requests an Hour-Long Host Wait.
        let (send, mut input) = support::invoke(&mut process.client, "execution.clock").await?;
        let sleep = support::child(&mut input).await?;
        assert_eq!(sleep.operation, "harmos.plugin.v1.Time/Sleep");
        assert_eq!(sleep.fingerprint, declarations.requires[2].fingerprint);
        assert_eq!(TimeSleepRequest::decode(sleep.payload.as_slice())?.duration, Some(prost_types::Duration { seconds: 3600, nanos: 0 }));

        // Complete the Wait by Protocol Event and Supply Literal Host Evidence Time.
        support::reply(&send, sleep.call, TimeSleepResponse {}).await?;
        let now = support::child(&mut input).await?;
        assert_eq!(now.operation, "harmos.plugin.v1.Time/Get");
        assert_eq!(now.fingerprint, declarations.requires[3].fingerprint);
        let at = Timestamp { seconds: 12345, nanos: 67890 };
        support::reply(&send, now.call, TimeGetResponse { now: Some(at) }).await?;
        let evidence = support::terminal::<Evidence>(&mut input).await?;
        assert_eq!(evidence.resource, 42);
        assert_eq!(evidence.at, Some(at));
        assert!(input.message().await?.is_none());

        // A Second Execution Cannot See the First Execution's Resources.
        let (send, mut input) = support::invoke(&mut process.client, "execution.isolation").await?;
        drop(send);
        assert!(support::terminal::<Isolation>(&mut input).await?.absent);
        assert!(input.message().await?.is_none());

        // Demand Closure Releases an Observer's In-Progress Device Read.
        let (demand, mut samples) = support::publish(&mut process.client, "execution.reading").await?;
        demand
            .send(InstanceSamplesRequest {
                frame: Some(instance_samples_request::Frame::Window(Window {
                    start: Some(at),
                    period: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
                    count: 1,
                    sequence: 1,
                })),
            })
            .await?;
        assert_eq!(process.event().await?, "observer_reading");
        drop(demand);
        assert!(samples.message().await?.is_none());
        let (_send, mut input) = support::invoke(&mut process.client, "execution.reading_released").await?;
        assert!(support::terminal::<Isolation>(&mut input).await?.absent);
        assert_eq!(fs::read(cleanup.with_extension("reading"))?, b"released");

        // Reject an Unset Frame During an Active Call Without Losing the Connection.
        let (send, mut input) = support::invoke(&mut process.client, "execution.clock").await?;
        support::child(&mut input).await?;
        send.send(InstanceInvokeRequest { frame: None }).await?;
        assert_eq!(input.message().await.unwrap_err().code(), tonic::Code::InvalidArgument);

        // Stop Completes Cleanup Before Replying and Keeps the Remote Process Available.
        process.client.stop(InstanceStopRequest { reason: "Story Finished".into() }).await?;
        assert_eq!(fs::read(&cleanup)?, b"cleaned");
        let second = support::describe(&mut process.client).await?;
        assert_eq!(first, second);
        process.close().await?;
        Ok(())
    };

    // Bound Only Transport Hangs; Scenario Time Is Entirely Host-Controlled.
    tokio::time::timeout(Duration::from_secs(30), story).await?
}
