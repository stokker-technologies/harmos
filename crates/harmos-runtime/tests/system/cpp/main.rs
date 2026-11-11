/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// One Focused Story Crosses The Real Host And The C++ Native Process.

/* === Modules === */

mod domain;
#[path = "../../support/report.rs"]
mod report;

/* === Re-Exports === */

/* === Imports === */

extern crate harmos_runtime as harmos;

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports
use futures_util::StreamExt;

// Local Imports
use domain::{Capture, Operations, ReadTemperature};
use harmos_domain::model::storage::ArrowIpc;
use harmos_domain::operations::WireDeclaration;
use harmos_runtime::config::{PluginSettings, PluginSidecarSettings};
use harmos_runtime::plugin::PluginState;
use harmos_runtime::sidecar::Executable;
use harmos_runtime::streams::Item;
use harmos_runtime::{Application, Lifecycle, Message, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Host;

/* === Enums === */

#[harmos_runtime::lanes]
enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */

impl Lifecycle for Host {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;
    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl Application for Host {
    type Lanes = Lanes;
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = ();
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn cpp_sidecar_walks() {
    let _report = report::system();
    tokio::time::timeout(std::time::Duration::from_secs(120), async {
        // Inspect Both Operations And Their Nominal Rust Contracts.
        let path = std::env::var_os("HARMOS_CPP_SIDECAR").expect("run mise run test:rust:cpp");
        let executable = Executable::at(path).unwrap();
        let declaration = executable.declaration();
        assert_eq!(declaration.id, "bench");
        let expected = [WireDeclaration::of::<ReadTemperature>().contract().unwrap(), WireDeclaration::of::<Capture>().contract().unwrap()];
        assert_eq!(declaration.contracts.provides, expected);
        assert_eq!(declaration.operations.len(), 2);
        assert_eq!(declaration.streams.len(), 1);

        // Launch The Real Plugin Host With The Existing Framed Carrier.
        let runtime = Runtime::<Host>::launch_with((), (), |setup| {
            setup.settings().operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: NonZeroUsize::new(1024 * 1024),
                ..Default::default()
            });
        })
        .await
        .unwrap();
        let settings = PluginSidecarSettings {
            plugin: PluginSettings {
                configuration: b"null".to_vec(),
                host_operations: vec![harmos_runtime::Identifier::new("host.publish").unwrap()],
                ..Default::default()
            },
            ..Default::default()
        };
        runtime.plugin.register_sidecar::<Operations>(executable, settings, Lanes::Open).await.unwrap();
        runtime.plugin.initialize().await.unwrap();
        assert_eq!(runtime.plugin.status("bench").unwrap().state, PluginState::Ready);

        // Receive A Typed Response From C++.
        let reading = runtime.operation(ReadTemperature { channel: 4 }).await.unwrap();
        assert_eq!(reading.temperature_c, 22.0);

        // Capture Invokes ReadTemperature Through The Host And Publishes Arrow.
        let publication = runtime.streams.current_publication("bench.temperature").unwrap();
        let mut rows = publication.watch();
        let captured = runtime.operation(Capture { channel: 4, sample_count: 3 }).await.unwrap();
        assert_eq!(captured.published, 3);
        let Item::Row(row) = rows.next().await.unwrap() else { panic!("publication window lost a batch") };
        let ipc = ArrowIpc::decode(&row.value.payload).unwrap();
        let mut reader = arrow_ipc::reader::StreamReader::try_new(std::io::Cursor::new(ipc.bytes()), None).unwrap();
        let batch = reader.next().unwrap().unwrap();
        assert_eq!(batch.num_rows(), 3);
        assert_eq!(batch.schema().field(0).name(), "channel");
        let channels = batch.column(0).as_any().downcast_ref::<arrow_array::Int32Array>().unwrap();
        let temperatures = batch.column(1).as_any().downcast_ref::<arrow_array::Float64Array>().unwrap();
        assert_eq!(channels.values().as_ref(), [4, 4, 4]);
        assert_eq!(temperatures.values().as_ref(), [22.0, 22.25, 22.5]);
        assert!(reader.next().is_none());

        // Wait For The Callback's Publication Before Requesting Cancellation.
        let capture = runtime.operations.submit::<Capture>(Capture { channel: 2, sample_count: 0 }).unwrap();
        let Item::Row(_) = rows.next().await.unwrap() else { panic!("publication window lost readiness") };
        capture.request_cancel();
        capture.settle().await.unwrap();
        assert!(capture.status().phase().is_terminal());
        assert!(capture.join().await.is_err());

        // A Foreign Exception Becomes A Permanent Operation Failure.
        let error = runtime.operation(ReadTemperature { channel: -1 }).await.unwrap_err();
        assert!(matches!(error, harmos_runtime::Error::OperationFailed { .. }));
        assert_eq!(error.failure().unwrap().code(), harmos_runtime::operations::FailureKind::Permanent);
        // A Successful Call Afterwards Proves The Process And Worker Remain Usable.
        assert_eq!(runtime.operation(ReadTemperature { channel: 6 }).await.unwrap().temperature_c, 23.0);

        // Retirement Settles The Native End Receipt, Which Joins The Physical Process.
        drop(rows);
        runtime.plugin.stop("bench").await.unwrap();
        let stopped = runtime.plugin.status("bench").unwrap();
        assert!(!stopped.cleanup_pending);
        assert_ne!(stopped.state, PluginState::Ready);
        assert!(runtime.operation(ReadTemperature { channel: 4 }).await.is_err());
        runtime.shutdown().await.unwrap();
    })
    .await
    .expect("the native sidecar story must settle within its hang guard");
}
