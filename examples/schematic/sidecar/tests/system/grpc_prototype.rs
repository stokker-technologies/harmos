/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Ordered Prototype Gates Over The Actual Schematic Sidecar Process.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::process::Stdio;
use std::sync::Arc;
// Third Party Imports
use arrow_array::{Float64Array, RecordBatch, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
// Local Imports
use harmos::OperationHost;
use harmos::builtins::lifecycle::{Initialize, Shutdown};
use harmos_sidecar::kit::prototype::{Host, Packet, Settings};
use schematic_operations::directories::ListDirectories;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Sidecar {
    child: tokio::process::Child,
    host: Host,
    directory: tempfile::TempDir,
    address: String,
    token: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Sidecar {
    async fn start(generation: u64) -> Self {
        // Prepare A Real Filesystem Operation.
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("beta")).unwrap();
        std::fs::create_dir(directory.path().join("alpha")).unwrap();
        std::fs::write(directory.path().join("ordinary-file"), b"ignored").unwrap();
        let executable = std::env::var_os("NEXTEST_BIN_EXE_schematic-grpc-prototype")
            .or_else(|| std::env::var_os("CARGO_BIN_EXE_schematic-grpc-prototype"))
            .expect("built prototype executable");
        let token = harmos_sidecar::kit::prototype::credentials().unwrap();
        let mut child = tokio::process::Command::new(executable)
            .current_dir(directory.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let configuration = serde_json::to_vec(&(token.clone(), generation)).unwrap();
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(&configuration).await.unwrap();
        stdin.write_all(b"\n").await.unwrap();
        stdin.flush().await.unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut address = String::new();
        stdout.read_line(&mut address).await.unwrap();
        let address = address.trim().to_owned();
        let host = Host::connect(&address, token.clone(), "schematic_tools".to_owned(), generation, settings()).await.unwrap();
        Self { child, host, directory, address, token }
    }
    async fn end(mut self) {
        self.child.kill().await.unwrap();
        self.child.wait().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn settings() -> Settings {
    Settings {
        message_bytes: NonZeroUsize::new(2 * 1024 * 1024).unwrap(),
        queued_batches: NonZeroUsize::new(2).unwrap(),
        window_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
        connection_window_bytes: NonZeroUsize::new(2 * 1024 * 1024).unwrap(),
        calls: NonZeroUsize::new(16).unwrap(),
    }
}

fn arrow(rows: usize) -> Vec<u8> {
    let schema =
        Arc::new(Schema::new(vec![Field::new("sequence", DataType::UInt64, false), Field::new("value", DataType::Float64, false)]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(UInt64Array::from_iter_values((0..rows).map(|row| row as u64))),
            Arc::new(Float64Array::from_iter_values((0..rows).map(|row| row as f64 * 0.25))),
        ],
    )
    .unwrap();
    let mut writer = arrow_ipc::writer::StreamWriter::try_new(Vec::new(), &schema).unwrap();
    writer.write(&batch).unwrap();
    writer.finish().unwrap();
    writer.into_inner().unwrap()
}

fn memory(process: u32) -> (u64, u64) {
    let script = format!("$p = Get-Process -Id {process}; Write-Output $p.PrivateMemorySize64; Write-Output $p.WorkingSet64");
    let output =
        std::process::Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", &script]).output().unwrap();
    assert!(output.status.success());
    let output = String::from_utf8(output.stdout).unwrap();
    let mut lines = output.lines();
    (lines.next().unwrap().trim().parse().unwrap(), lines.next().unwrap().trim().parse().unwrap())
}

#[tokio::test]
async fn gate_1_real_sidecar() {
    // One Whole-Story Hang Guard; No Timing Assertion.
    tokio::time::timeout(std::time::Duration::from_secs(120), async {
        // Start The Authenticated Child And Initialize Its Existing SDK Owner.
        let sidecar = Sidecar::start(1).await;
        sidecar
            .host
            .operation(Initialize {
                config_json: b"null".to_vec(),
                generation_context: vec![7; 32],
                assignments: harmos_sidecar::operations::AssignedOperation::encode(&[harmos_sidecar::operations::AssignedOperation {
                    operation: "host.host.publish".into(),
                    fingerprint: harmos::builtins::streams::Publish::definition().fingerprint.0,
                    generation: 1,
                }])
                .unwrap(),
            })
            .await
            .unwrap();

        // Invoke The Existing Native Operation Through Invocation.
        let result = sidecar.host.operation(ListDirectories {}).await.unwrap();
        assert_eq!(result.directories, ["alpha", "beta"]);

        // Carry The Same Arrow IPC Batch Through The Separate Data RPC.
        let payload = arrow(4096);
        let mut incoming = sidecar.host.arrow(100, payload.clone(), 3).await.unwrap();
        for sequence in 0..3 {
            let received = incoming.message().await.unwrap().unwrap();
            assert_eq!(received.count, sequence);
            assert_eq!(received.payload, payload);
            let mut reader = arrow_ipc::reader::StreamReader::try_new(std::io::Cursor::new(received.payload), None).unwrap();
            assert_eq!(reader.next().unwrap().unwrap().num_rows(), 4096);
        }
        assert!(incoming.message().await.unwrap().is_none());

        // Shutdown Through The Same Operation Contract And Retire The Generation.
        sidecar.host.operation(Shutdown {}).await.unwrap();
        sidecar.host.retire().await.unwrap();
        assert!(sidecar.directory.path().join("alpha").is_dir());
        sidecar.end().await;
    })
    .await
    .expect("prototype story hang guard");
}

#[tokio::test]
async fn gate_2_cancellation_under_backpressure() {
    tokio::time::timeout(std::time::Duration::from_secs(120), async {
        // Start The Same Sidecar And Leave Its Data Response Unread.
        let sidecar = Sidecar::start(2).await;
        let payload = arrow(4096);
        for invocation in 1..=8 {
            let incoming = sidecar.host.arrow(invocation, payload.clone(), 65_536).await.unwrap();
            let blocked = sidecar
                .host
                .phase(Packet {
                    phase: 11,
                    invocation,
                    ..Packet::default()
                })
                .await
                .unwrap();
            assert_eq!(blocked.phase, 0);
            assert!(blocked.count < 65_536);

            // Await Cancellation Settlement Without Reading Or Dropping The Data Response.
            let started = std::time::Instant::now();
            let settled = sidecar
                .host
                .phase(Packet {
                    phase: 10,
                    invocation,
                    ..Packet::default()
                })
                .await
                .unwrap();
            let cancellation_ms = started.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(settled.phase, 1);
            assert!(settled.count < 65_536);
            let after = sidecar
                .host
                .phase(Packet {
                    phase: 12,
                    invocation,
                    ..Packet::default()
                })
                .await
                .unwrap();
            assert_eq!(after.count, settled.count);
            eprintln!(
                "GATE_2 repetition={invocation} payload_bytes={} produced={} cancellation_ms={cancellation_ms:.3}",
                payload.len(),
                settled.count
            );
            drop(incoming);
        }

        // A New Transfer Completes After The Blocked Producers Settle.
        let mut incoming = sidecar.host.arrow(100, payload.clone(), 1).await.unwrap();
        assert_eq!(incoming.message().await.unwrap().unwrap().payload, payload);
        assert!(incoming.message().await.unwrap().is_none());
        sidecar.host.retire().await.unwrap();
        sidecar.end().await;
    })
    .await
    .expect("prototype story hang guard");
}

#[tokio::test]
async fn gate_3_restart_and_retirement() {
    tokio::time::timeout(std::time::Duration::from_secs(120), async {
        // Preserve A Real Startup Failure Through The Structured Error Boundary.
        let invalid = Sidecar::start(30).await;
        let error = invalid.host.operation(Initialize {
            config_json: b"{".to_vec(), generation_context: vec![7; 32], assignments: Vec::new(),
        }).await.unwrap_err();
        let harmos_sidecar::kit::prototype::Error::Failure(failure) = error else { panic!("structured initialization failure") };
        eprintln!("GATE_3 startup_failure={}", serde_json::to_string(&failure).unwrap());
        assert!(invalid.host.operation(ListDirectories {}).await.is_err());
        invalid.host.retire().await.unwrap();
        invalid.end().await;

        // Initialize The Next Physical Generation With The Unchanged Catalog.
        let sidecar = Sidecar::start(31).await;
        sidecar.host.operation(Initialize {
            config_json: b"null".to_vec(), generation_context: vec![7; 32],
            assignments: harmos_sidecar::operations::AssignedOperation::encode(&[
                harmos_sidecar::operations::AssignedOperation {
                    operation: "host.host.publish".into(),
                    fingerprint: harmos::builtins::streams::Publish::definition().fingerprint.0,
                    generation: 1,
                },
            ]).unwrap(),
        }).await.unwrap();
        assert_eq!(sidecar.host.operation(ListDirectories {}).await.unwrap().directories, ["alpha", "beta"]);

        // Keep A Prepared Operation And An Unread Data Stream Live During Retirement.
        let prepared = sidecar.host.phase(Packet {
            phase: 1, invocation: 500, operation: format!("schematic_tools.{}", ListDirectories::definition().id),
            fingerprint: ListDirectories::definition().fingerprint.0.to_vec(), context: vec![1; 32], payload: Vec::new().into(),
            ..Packet::default()
        }).await.unwrap();
        assert_eq!(prepared.phase, 0);
        let incoming = sidecar.host.arrow(600, arrow(4096), 65_536).await.unwrap();
        sidecar.host.phase(Packet { phase: 11, invocation: 600, ..Packet::default() }).await.unwrap();
        sidecar.host.retire().await.unwrap();
        let retired = sidecar.host.phase(Packet { phase: 3, invocation: 500, context: vec![1; 32], ..Packet::default() }).await.unwrap_err();
        assert!(matches!(retired, harmos_sidecar::kit::prototype::Error::Status(status) if status.code() == harmos_sidecar::kit::prototype::tonic::Code::FailedPrecondition));
        assert!(sidecar.host.operation(ListDirectories {}).await.is_err());
        let old_token = sidecar.token.clone();
        drop(incoming);
        sidecar.end().await;

        // Restart And Refuse Both Retired Credentials And A Mismatched Generation.
        let restarted = Sidecar::start(32).await;
        let old = Host::connect(&restarted.address, old_token, "schematic_tools".into(), 31, settings()).await;
        assert!(matches!(old, Err(harmos_sidecar::kit::prototype::Error::Status(status)) if status.code() == harmos_sidecar::kit::prototype::tonic::Code::Unauthenticated));
        let stale = Host::connect(&restarted.address, restarted.token.clone(), "schematic_tools".into(), 31, settings()).await;
        assert!(matches!(stale, Err(harmos_sidecar::kit::prototype::Error::Status(status)) if status.code() == harmos_sidecar::kit::prototype::tonic::Code::FailedPrecondition));
        restarted.host.operation(Initialize {
            config_json: b"null".to_vec(), generation_context: vec![8; 32],
            assignments: harmos_sidecar::operations::AssignedOperation::encode(&[
                harmos_sidecar::operations::AssignedOperation {
                    operation: "host.host.publish".into(),
                    fingerprint: harmos::builtins::streams::Publish::definition().fingerprint.0,
                    generation: 1,
                },
            ]).unwrap(),
        }).await.unwrap();
        assert_eq!(restarted.host.operation(ListDirectories {}).await.unwrap().directories, ["alpha", "beta"]);
        restarted.host.operation(Shutdown {}).await.unwrap();
        restarted.host.retire().await.unwrap();
        restarted.end().await;
        eprintln!("GATE_3 startup_refusal=preserved generations=31,32 prepared_call=fenced retired_token=refused stale_generation=refused shutdown=settled");
    }).await.expect("prototype story hang guard");
}

#[tokio::test]
async fn gate_4_slow_consumer_memory() {
    tokio::time::timeout(std::time::Duration::from_secs(120), async {
        // Keep The Existing Queue, Message Ceiling And Fixed Stream Window.
        let sidecar = Sidecar::start(40).await;
        let child = sidecar.child.id().unwrap();
        let policy = settings();
        let allowance = policy.message_bytes.get() as u64 * (policy.queued_batches.get() as u64 + 2);
        let mut invocation = 0;
        for rows in [4096, 65_536] {
            let payload = arrow(rows);

            // Warm The Same Fixed Pipeline Before Comparing Transfer Sizes.
            invocation += 1;
            let mut incoming = sidecar.host.arrow(invocation, payload.clone(), 8).await.unwrap();
            while let Some(packet) = incoming.message().await.unwrap() {
                assert_eq!(packet.payload, payload);
            }
            drop(incoming);
            let baseline = memory(child);
            let parent_baseline = memory(std::process::id());
            let mut private_peak = baseline.0;
            let mut parent_private_peak = parent_baseline.0;
            for count in [64, 4096, 65_536] {
                // Windows Process Sampling Makes This Consumer Deliberately Slow.
                invocation += 1;
                let mut incoming = sidecar.host.arrow(invocation, payload.clone(), count).await.unwrap();
                for consumed in 0..8 {
                    let blocked = sidecar.host.phase(Packet { phase: 11, invocation, ..Packet::default() }).await.unwrap();
                    assert_eq!(blocked.phase, 0);
                    assert!(blocked.count <= consumed + policy.queued_batches.get() as u64 + 2);
                    let sample = memory(child);
                    let parent = memory(std::process::id());
                    private_peak = private_peak.max(sample.0);
                    parent_private_peak = parent_private_peak.max(parent.0);
                    eprintln!("GATE_4 payload_bytes={} requested={count} consumed={consumed} produced={} child_private={} child_working_set={} parent_private={}", payload.len(), blocked.count, sample.0, sample.1, parent.0);
                    let packet = incoming.message().await.unwrap().unwrap();
                    assert_eq!(packet.count, consumed);
                    assert_eq!(packet.payload, payload);
                }
                sidecar.host.phase(Packet { phase: 10, invocation, ..Packet::default() }).await.unwrap();
                drop(incoming);
            }
            let growth = private_peak.saturating_sub(baseline.0);
            let parent_growth = parent_private_peak.saturating_sub(parent_baseline.0);
            eprintln!("GATE_4_SUMMARY payload_bytes={} child_baseline={} child_peak={private_peak} child_growth={growth} parent_baseline={} parent_peak={parent_private_peak} parent_growth={parent_growth} pipeline_allowance={allowance}", payload.len(), baseline.0, parent_baseline.0);
            assert!(growth <= allowance, "child memory must stay within the fixed pipeline envelope");
            assert!(parent_growth <= allowance, "consumer memory must stay within the fixed pipeline envelope");
        }
        sidecar.host.retire().await.unwrap();
        sidecar.end().await;
    }).await.expect("prototype story hang guard");
}

/* === Tests === */
