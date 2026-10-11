/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::time::Duration;
// Third Party Imports

// Local Imports
use crate::error::Error;
use harmos::{OperationDeclaration, StreamSpec};
use minibench_app::app;
use minibench_app::config::Config;
use minibench_app::operations::{Corrected, ReadSample, Report, ReportRequest, Sample};
use minibench_app::services::probe;
use minibench_app::streams::Reading;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub async fn run(config: &Config) -> Result<String, Error> {
    let declaration = probe::inspect(config)?;
    assert_eq!(declaration.id, probe::NAME);
    assert_eq!(
        declaration
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>(),
        [ReadSample::ID]
    );
    // Check the remote publication mapping used by ingest. Its qualified wire
    // identity differs from the host-owned Reading stream's local identity.
    assert_eq!(
        declaration
            .streams
            .iter()
            .map(|stream| (stream.id.as_str(), stream.schema, stream.window))
            .collect::<Vec<_>>(),
        [(
            probe::PublishedReading::ID,
            probe::PublishedReading::VERSION,
            probe::PublishedReading::WINDOW
        )]
    );

    // The rig is resolved once, before boot, and borrowed by every attempt of
    // every operation: the device this bench is wired to, the calibration it was
    // measured with, and the handle its bridge answers on are true for the
    // runtime's whole life, so none of them belongs in an operation's request. The
    // bridge itself is declared at build, beside the catalogs.
    let mut runtime = app::launch(config).await?;

    let mut batches = runtime.streams.batches::<Reading>();
    let sampled = runtime
        .operations
        .call::<ReadSample>(Sample { channel: 1 })
        .await?;
    let millivolts = sampled.millivolts;
    assert_eq!(millivolts, 3_310);
    let reading = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(reading) = runtime.streams.query::<Reading>(..).await?.next() {
                break Ok::<_, Error>(reading.value);
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .map_err(|_| Error::PublicationTimeout)??;
    assert_eq!(
        reading,
        Reading {
            device: config.device().to_owned(),
            channel: 1,
            millivolts: 3_310,
        }
    );
    // The first raw row was already available before the three-row group filled.
    for channel in [2, 3] {
        runtime
            .operations
            .call::<ReadSample>(Sample { channel })
            .await?;
    }
    let summary = tokio::time::timeout(Duration::from_secs(5), batches.next())
        .await
        .map_err(|_| Error::PublicationTimeout)?;
    let Some(harmos::BatchItem::Summary(summary)) = summary else {
        panic!("three received rows produce one group")
    };
    assert_eq!(summary.count, 3);
    assert!(summary.complete());
    let stats = summary.fields[0]
        .statistics
        .expect("finite voltage statistics");
    assert_eq!((stats.min, stats.mean, stats.max), (3310.0, 3320.0, 3330.0));
    assert_eq!(runtime.streams.query::<Reading>(..).await?.count(), 3);

    // The operation owns its captured readings while the shared resource owner
    // supplies device identity and calibration on the declared serial lane.
    let captured: Vec<Reading> = runtime
        .streams
        .query::<Reading>(..)
        .await?
        .map(|row| row.value)
        .collect();
    let corrected = runtime
        .operations
        .call::<Report>(ReportRequest {
            readings: captured,
            channel: 2,
        })
        .await?;
    assert_eq!(
        *corrected,
        Corrected {
            device: config.device().to_owned(),
            channel: 2,
            millivolts: 3_310,
        }
    );

    let stopped = runtime.stop().await?;

    Ok(format!(
        concat!(
            "1  inspect -> probe-bridge metadata read without starting native code\n",
            "2  declare -> executable attached at build as a resident sidecar\n",
            "3  initialize -> sidecar started for {device}\n",
            "4  sample(channel 1) -> 3310 mV and one declared reading\n",
            "5  samples(channels 1–3) -> mean 3320 mV; raw delivery stays immediate\n",
            "6  report(channel 2) -> corrected against the boot-declared calibration\n",
            "7  stop -> native child ended; journal settled at position {stopped}\n",
        ),
        device = config.device(),
        stopped = stopped,
    ))
}

/* === Private Functions === */

/* === Tests === */
