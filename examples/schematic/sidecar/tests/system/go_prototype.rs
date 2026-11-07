/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Rust Host Reviews the Go SDK Through the Existing Prototype

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

// Third Party Imports
use prost::Message;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

// Local Imports
use harmos::OperationHost;
use harmos::builtins::lifecycle::{Initialize, Shutdown};
use harmos_sidecar::kit::prototype::{Host, Packet, SCHEMA_FINGERPRINT, Settings, protocol};

/* === Constants === */

const POLICY: harmos::operations::OperationPolicy = harmos::operations::OperationPolicy {
    timeout: Duration::from_secs(120),
    attempt_timeout: Duration::from_secs(120),
    cancellation_timeout: Duration::ZERO,
    retries: harmos::operations::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn rust_host_go_sidecar() {
    tokio::time::timeout(POLICY.timeout, async {
        // Launch The Go Example With The Existing Private Bootstrap.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let executable = root.join("target").join(format!("go-sidecar{}", std::env::consts::EXE_SUFFIX));
        let token = harmos_sidecar::kit::prototype::credentials().unwrap();
        let generation = 51;
        let mut child = tokio::process::Command::new(executable)
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
        let settings = Settings {
            message_bytes: NonZeroUsize::new(2 * 1024 * 1024).unwrap(),
            queued_batches: NonZeroUsize::new(2).unwrap(),
            window_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
            connection_window_bytes: NonZeroUsize::new(2 * 1024 * 1024).unwrap(),
            calls: NonZeroUsize::new(16).unwrap(),
        };
        let host = Host::connect(address.trim(), token.clone(), "go_tools".into(), generation, settings).await.unwrap();

        // Refuse Wrong Credentials And Stale Generations.
        assert!(Host::connect(address.trim(), "0".repeat(64), "go_tools".into(), generation, settings).await.is_err());
        assert!(Host::connect(address.trim(), token, "go_tools".into(), generation - 1, settings).await.is_err());

        // Read The Declaration And Initialize Through The Existing Operation Host.
        let welcome = host.phase(Packet::default()).await.unwrap();
        assert_eq!(welcome.declarations.len(), 1);
        let declaration = &welcome.declarations[0];
        assert_eq!(declaration.operation, "go_tools.greet");
        assert_eq!(declaration.fingerprint, SCHEMA_FINGERPRINT);
        host.operation(Initialize {
            config_json: b"null".to_vec(),
            generation_context: vec![7; 32],
            assignments: Vec::new(),
        })
        .await
        .unwrap();

        // Prepare, Initialize, Attempt And Finalize The One Generated Operation.
        let payload = protocol::GreetRequest { name: "SDK Author".into() }.encode_to_vec();
        host.phase(Packet {
            phase: protocol::Phase::Prepare as u32,
            invocation: 100,
            operation: declaration.operation.clone(),
            fingerprint: declaration.fingerprint.clone(),
            context: vec![1; 32],
            payload: payload.into(),
            ..Packet::default()
        })
        .await
        .unwrap();
        host.phase(Packet {
            phase: protocol::Phase::Initialize as u32,
            invocation: 100,
            ..Packet::default()
        })
        .await
        .unwrap();
        let result = host
            .phase(Packet {
                phase: protocol::Phase::Attempt as u32,
                invocation: 100,
                ..Packet::default()
            })
            .await
            .unwrap();
        let response = protocol::GreetResponse::decode(result.payload).unwrap();
        assert_eq!(response.greeting, "Hello, SDK Author!");
        host.phase(Packet {
            phase: protocol::Phase::Finalize as u32,
            invocation: 100,
            ..Packet::default()
        })
        .await
        .unwrap();

        // Consume Ordered, Byte-Identical Arrow IPC Through The Data RPC.
        let payload = super::grpc_prototype::arrow(4096);
        let mut incoming = host.arrow(200, payload.clone(), 3).await.unwrap();
        for sequence in 0..3 {
            let batch = incoming.message().await.unwrap().unwrap();
            assert_eq!(batch.invocation, 200);
            assert_eq!(batch.count, sequence);
            assert_eq!(batch.payload, payload);
            let mut reader = arrow_ipc::reader::StreamReader::try_new(std::io::Cursor::new(batch.payload), None).unwrap();
            let batch = reader.next().unwrap().unwrap();
            assert_eq!(batch.num_rows(), 4096);
        }
        assert!(incoming.message().await.unwrap().is_none());

        // Cancel An Unread Response Without Draining Or Dropping It.
        let incoming = host.arrow(201, payload.clone(), 65_536).await.unwrap();
        let settled = host
            .phase(Packet {
                phase: protocol::Phase::CancelData as u32,
                invocation: 201,
                ..Packet::default()
            })
            .await
            .unwrap();
        assert_eq!(settled.phase, 1);
        assert!(settled.count < 65_536);
        let after = host
            .phase(Packet {
                phase: protocol::Phase::Progress as u32,
                invocation: 201,
                ..Packet::default()
            })
            .await
            .unwrap();
        assert_eq!(after.phase, 1);
        assert_eq!(after.count, settled.count);
        drop(incoming);

        // Complete Another Transfer, Shut Down And Observe Physical Process Exit.
        let mut incoming = host.arrow(202, payload.clone(), 1).await.unwrap();
        assert_eq!(incoming.message().await.unwrap().unwrap().payload, payload);
        assert!(incoming.message().await.unwrap().is_none());
        host.operation(Shutdown {}).await.unwrap();
        host.retire().await.unwrap();
        assert!(child.wait().await.unwrap().success());
        eprintln!("GO_SLICE greeting={:?} arrow_batches=3 cancelled_produced={} shutdown=settled", response.greeting, settled.count);
    })
    .await
    .expect("Go sidecar story policy");
}

/* === Tests === */
