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
use harmos_sidecar::kit::prototype::{Host, Settings};
use schematic_operations::directories::ListDirectories;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Sidecar {
    child: tokio::process::Child,
    host: Host,
    directory: tempfile::TempDir,
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
        let host = Host::connect(address.trim(), token, "schematic_tools".to_owned(), generation, settings()).await.unwrap();
        Self { child, host, directory }
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
                assignments: Vec::new(),
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

/* === Tests === */
