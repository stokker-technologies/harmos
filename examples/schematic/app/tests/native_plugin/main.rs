/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Edit Advice Crosses gRPC While Streaming Analysis Stays in the Host

/* === Modules === */

mod support;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use futures_util::StreamExt;

// Local Imports
use schematic_app::{operations::native, services::sidecar};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn native_advice_and_host_streaming_share_one_document() {
    // Inspect and Package the Native Plugin Without an Embedded Inventory
    let binary = support::binary();
    let inspection = harmos_packaging::inspect(&binary).unwrap();
    assert_eq!(inspection.id, "schematic_tools");
    assert_eq!(inspection.operations, ["schematic.validate", "schematic.route", "schematic.list.directories"]);
    let bundle = support::package(binary);
    assert_eq!(bundle.manifest().identity.name, "schematic_tools");
    assert!(bundle.manifest().files[0].executable.as_ref().unwrap().registrations.is_empty());
    let staging = tempfile::tempdir().unwrap();
    let installed = bundle.extract(staging.path()).unwrap();
    let binary = installed.path().join(&bundle.manifest().files[0].path);

    // Attach the Managed SDK Plugin
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("zeta")).unwrap();
    std::fs::create_dir(directory.path().join("alpha")).unwrap();
    std::fs::write(directory.path().join("notes.txt"), b"Not a Directory").unwrap();
    let runtime = support::host(directory.path()).await;
    let plugin = sidecar::start(&runtime, &binary, directory.path()).await.unwrap();

    // List Only Directories Through the Plugin
    let directories = sidecar::call(&runtime, native::ListDirectories {}).await.unwrap();
    assert_eq!(directories.directories, ["alpha", "zeta"]);

    // Validate and Route the Document Through Unary Calls
    let request = native::ValidateSchematic { schematic: support::snapshot().into() };
    let validation = sidecar::call(&runtime, request).await.unwrap();
    assert!(validation.diagnostics.is_empty());
    let request = native::ValidateSchematic {
        schematic: native::Snapshot {
            symbols: vec![1],
            connections: vec![native::Connection { from: 1, to: 2 }],
        },
    };
    assert!(!sidecar::call(&runtime, request).await.unwrap().diagnostics.is_empty());
    let request = native::ProposeRoute {
        from: native::Point { x: 0, y: 0 },
        to: native::Point { x: 3, y: 4 },
    };
    let route = sidecar::call(&runtime, request).await.unwrap();
    assert_eq!(route.points, [native::Point { x: 0, y: 0 }, native::Point { x: 3, y: 0 }, native::Point { x: 3, y: 4 }]);

    // Upload Analysis and Observe Host-Owned Progress
    let mut progress = runtime.streams.watch::<schematic_app::domain::model::schematic::progress::AnalysisProgress>();
    let request = schematic_app::operations::AnalyzeUpload { title: "Board".into() };
    let mut upload = runtime.operations.upload::<schematic_app::operations::AnalyzeUpload>(request, support::settings()).unwrap();
    upload.send(support::snapshot()).await.unwrap();
    upload.finish_input();
    assert_eq!(upload.join().await.unwrap().checked, 1);
    let harmos::streams::Item::Row(row) = progress.next().await.unwrap() else { panic!("Expected Analysis Progress") };
    assert_eq!((row.value.title.as_str(), row.value.checked), ("Board", 1));

    // Exchange Snapshots and Findings in the Host
    let request = schematic_app::operations::AnalyzeExchange { title: "Board".into() };
    let mut exchange = runtime
        .operations
        .exchange::<schematic_app::operations::AnalyzeExchange>(request, support::settings(), support::settings())
        .unwrap();
    exchange.send(support::snapshot()).await.unwrap();
    exchange.finish_input();
    assert!(exchange.next().await.unwrap().is_some());
    assert!(exchange.next().await.unwrap().is_none());
    assert_eq!(exchange.join().await.unwrap().checked, 1);
    let harmos::streams::Item::Row(row) = progress.next().await.unwrap() else { panic!("Expected Exchange Progress") };
    assert_eq!((row.value.title.as_str(), row.value.checked), ("Board", 1));

    // Reject Invalid Analysis and Settle Cancellation Before Reusing Its Permit
    let request = schematic_app::operations::AnalyzeUpload { title: String::new() };
    let mut invalid = runtime.operations.upload::<schematic_app::operations::AnalyzeUpload>(request, support::settings()).unwrap();
    invalid.finish_input();
    let failure = invalid.join().await.unwrap_err();
    let harmos::Error::OperationFailed { source, .. } = failure else { panic!("Expected Authored Input Failure: {failure:?}") };
    assert!(matches!(source.downcast_ref::<schematic_app::operations::Error>(), Some(schematic_app::operations::Error::InvalidInput)));
    let request = schematic_app::operations::AnalyzeUpload { title: "Cancelled".into() };
    let cancelled = runtime.operations.upload::<schematic_app::operations::AnalyzeUpload>(request, support::settings()).unwrap();
    cancelled.cancel_and_settle().await.unwrap();
    let request = schematic_app::operations::AnalyzeUpload { title: "Resumed".into() };
    let mut resumed = runtime.operations.upload::<schematic_app::operations::AnalyzeUpload>(request, support::settings()).unwrap();
    resumed.send(support::snapshot()).await.unwrap();
    resumed.finish_input();
    assert_eq!(resumed.join().await.unwrap().checked, 1);
    let harmos::streams::Item::Row(row) = progress.next().await.unwrap() else { panic!("Expected Resumed Progress") };
    assert_eq!(row.value.title, "Resumed");

    // Hold the Shared Permit Until Upload Cancellation Settles
    let request = schematic_app::operations::AnalyzeUpload { title: "Held".into() };
    let held = runtime.operations.upload::<schematic_app::operations::AnalyzeUpload>(request, support::settings()).unwrap();
    held.send(support::snapshot()).await.unwrap();
    let (input, (), completion) = held.split();
    let harmos::streams::Item::Row(row) = progress.next().await.unwrap() else { panic!("Expected Held Progress") };
    assert_eq!(row.value.title, "Held");
    let request = schematic_app::operations::AnalyzeExchange { title: "Waiting".into() };
    let mut waiting = runtime
        .operations
        .exchange::<schematic_app::operations::AnalyzeExchange>(request, support::settings(), support::settings())
        .unwrap();
    waiting.send(support::snapshot()).await.unwrap();
    {
        let next = waiting.next();
        futures_util::pin_mut!(next);
        assert!(futures_util::poll!(next).is_pending());
    }
    completion.cancel_and_settle().await.unwrap();
    drop(input);
    assert!(waiting.next().await.unwrap().is_some());
    waiting.finish_input();
    assert!(waiting.next().await.unwrap().is_none());
    assert_eq!(waiting.join().await.unwrap().checked, 1);
    let harmos::streams::Item::Row(row) = progress.next().await.unwrap() else { panic!("Expected Waiting Progress") };
    assert_eq!(row.value.title, "Waiting");

    // Upload and Download Transfer Batches Through Host-Owned Streams
    let batch = schematic_app::domain::model::transfer::Batch {
        records: vec![schematic_app::domain::model::transfer::Record { sequence: 1, payload: vec![0, 255] }],
        samples: vec![schematic_app::domain::model::transfer::Sample {
            sequence: 2,
            timestamp_us: 123,
            value: 1.5,
        }],
        bytes: vec![0, 1, 255],
    };
    let request = schematic_app::operations::transfer::Upload { transfer: 7 };
    let mut upload = runtime.operations.upload::<schematic_app::operations::transfer::Upload>(request, support::settings()).unwrap();
    upload.send(batch).await.unwrap();
    upload.finish_input();
    let receipt = upload.join().await.unwrap();
    assert_eq!((receipt.records, receipt.samples, receipt.bytes), (1, 1, 3));

    // Unary Transfer Calls Share the Host Store With Streaming Calls
    let request = schematic_app::operations::transfer::Put {
        transfer: 8,
        first: true,
        batch: schematic_app::domain::model::transfer::Batch {
            records: Vec::new(),
            samples: Vec::new(),
            bytes: vec![3, 2, 1],
        },
    };
    assert_eq!(runtime.operation(request).await.unwrap().bytes, 3);
    let request = schematic_app::operations::transfer::Take { transfer: 8 };
    assert_eq!(runtime.operation(request).await.unwrap().bytes, [3, 2, 1]);
    let request = schematic_app::operations::transfer::Download { transfer: 7 };
    let mut download = runtime.operations.download::<schematic_app::operations::transfer::Download>(request, support::settings()).unwrap();
    let batch = download.next().await.unwrap().unwrap();
    assert_eq!((batch.records[0].sequence, batch.records[0].payload.as_slice()), (1, &[0, 255][..]));
    assert_eq!((batch.samples[0].sequence, batch.samples[0].timestamp_us, batch.samples[0].value), (2, 123, 1.5));
    assert_eq!(batch.bytes, [0, 1, 255]);
    assert!(download.next().await.unwrap().is_none());
    let receipt = download.join().await.unwrap();
    assert_eq!((receipt.records, receipt.samples, receipt.bytes), (1, 1, 3));

    // Stop Withdraws Plugin Advice While Host Analysis Remains Available
    plugin.stop("Story Complete").await.unwrap();
    assert!(sidecar::call(&runtime, native::ListDirectories {}).await.is_err());
    let request = schematic_app::operations::AnalyzeSchematic { schematic: support::snapshot() };
    let mut analysis = runtime.operations.download::<schematic_app::operations::AnalyzeSchematic>(request, support::settings()).unwrap();
    let mut findings = Vec::new();
    while let Some(finding) = analysis.next().await.unwrap() {
        findings.push(finding.text.clone());
    }
    assert_eq!(findings.len(), 3);
    assert_eq!(analysis.join().await.unwrap().checked, 3);
    runtime.shutdown().await.unwrap();
}
