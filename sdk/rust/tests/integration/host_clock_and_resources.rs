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

// Third Party Imports
use prost::Message as _;

// Local Imports
use super::support;
use harmos_protocol::v1;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(prost::Message)]
struct Evidence {
    #[prost(uint64, tag = "1")]
    resource: u64,

    #[prost(message, optional, tag = "2")]
    at: Option<prost_types::Timestamp>,
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
async fn host_clock_and_resources() -> Result<(), Box<dyn std::error::Error>> {
    let story = async {
        // Start the Remote SDK Plugin and Admit Its Clock Requirements.
        let scratch = tempfile::tempdir()?;
        let cleanup = scratch.path().join("cleanup");
        let mut process = support::Process::start(&cleanup).await?;
        let first = support::describe(&mut process.client).await?;
        let catalog = first.catalog.as_ref().ok_or("Missing Catalog")?;
        assert_eq!(
            catalog.requires.iter().map(|requirement| requirement.id.as_str()).collect::<Vec<_>>(),
            ["harmos.time.sleep", "harmos.time.get"]
        );

        // Begin an Execution That Stores a Resource and Requests an Hour-Long Host Wait.
        let (send, mut input) = support::invoke(&mut process.client, "execution.clock").await?;
        let sleep = support::child(&mut input).await?;
        assert_eq!(sleep.operation, "harmos.time.sleep");
        assert_eq!(sleep.fingerprint, catalog.requires[0].fingerprint);
        assert_eq!(
            v1::TimeSleepRequest::decode(sleep.payload.as_slice())?.duration,
            Some(prost_types::Duration { seconds: 3600, nanos: 0 })
        );

        // Complete the Wait by Protocol Event and Supply Literal Host Evidence Time.
        support::reply(&send, sleep.call, v1::TimeSleepResponse {}).await?;
        let now = support::child(&mut input).await?;
        assert_eq!(now.operation, "harmos.time.get");
        assert_eq!(now.fingerprint, catalog.requires[1].fingerprint);
        let at = prost_types::Timestamp { seconds: 12345, nanos: 67890 };
        support::reply(&send, now.call, v1::TimeGetResponse { now: Some(at) }).await?;
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
            .send(v1::PluginSamplesRequest {
                frame: Some(v1::plugin_samples_request::Frame::Tick(v1::Tick { at: Some(at) })),
            })
            .await?;
        assert_eq!(process.event().await?, "observer_reading");
        drop(demand);
        assert!(samples.message().await?.is_none());
        let (_send, mut input) = support::invoke(&mut process.client, "execution.reading_released").await?;
        assert!(support::terminal::<Isolation>(&mut input).await?.absent);
        assert_eq!(std::fs::read(cleanup.with_extension("reading"))?, b"released");

        // Reject an Unset Frame During an Active Call Without Losing the Connection.
        let (send, mut input) = support::invoke(&mut process.client, "execution.clock").await?;
        support::child(&mut input).await?;
        send.send(v1::PluginInvokeRequest { frame: None }).await?;
        assert_eq!(input.message().await.unwrap_err().code(), tonic::Code::InvalidArgument);

        // Stop Completes Cleanup Before Replying and Keeps the Remote Process Available.
        process.client.stop(v1::PluginStopRequest { reason: "Story Finished".into() }).await?;
        assert_eq!(std::fs::read(&cleanup)?, b"cleaned");
        let second = support::describe(&mut process.client).await?;
        assert_eq!(first, second);
        process.close().await?;
        Ok(())
    };

    // Bound Only Transport Hangs; Scenario Time Is Entirely Host-Controlled.
    tokio::time::timeout(std::time::Duration::from_secs(30), story).await?
}
