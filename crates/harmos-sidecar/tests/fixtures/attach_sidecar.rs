/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod manifest {
    use harmos_sidecar::inventory::contracts::{encode_tree, encoded_tree_len};
    use harmos_sidecar::operations::{OperationPolicy, RestartPolicy, RetryPolicy};
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: std::time::Duration::from_secs(3),
        attempt_timeout: std::time::Duration::from_secs(3),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const SDK: [harmos_sidecar::operations::OperationPolicy; 2] = [POLICY, POLICY];
    pub(super) const LEN: usize = encoded_tree_len("sidecar", "attach-fixture", &[], &[], &SDK);
    pub(super) const ENCODED: [u8; LEN] = encode_tree("sidecar", "attach-fixture", &[], &[], &SDK);
}
mod publications {
    use harmos_sidecar::inventory::publications::{Entry, encode, encoded_len};
    const STREAMS: &[Entry] = &[];
    pub(super) const LEN: usize = encoded_len("sidecar", "attach-fixture", STREAMS);
    pub(super) const ENCODED: [u8; LEN] = encode("sidecar", "attach-fixture", STREAMS);
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;
// Third Party Imports

// Local Imports

/* === Constants === */
// Handwritten Identity Exercises Inspection Of Independently Authored Bytes
const DECLARATION: &[u8; 155] = b"\0HARMOS:ARTIFACT:V6:{\"kind\":\"plugin\",\"facility\":\"sidecar\",\"id\":\"attach-fixture\",\"version\":\"0.2.0\",\"description\":\"Checks native attachment.\",\"config\":\"()\"}\0";
const RESTART: &[u8; 180] = b"\0HARMOS:ARTIFACT:V6:{\"kind\":\"restart\",\"facility\":\"sidecar\",\"owner\":\"attach-fixture\",\"policy\":{\"credit\":0,\"sustained_health_ms\":30000,\"backoff_base_ms\":250,\"backoff_cap_ms\":10000}}\0";

#[used]
static HARMOS_DECLARATION: [u8; DECLARATION.len()] = *DECLARATION;

#[used]
static HARMOS_RESTART: [u8; RESTART.len()] = *RESTART;

#[used]
static MANIFEST: [u8; manifest::LEN] = manifest::ENCODED;

#[used]
static PUBLICATIONS: [u8; publications::LEN] = publications::ENCODED;

#[used]
static OBSERVERS: [u8; harmos_sidecar::inventory::observers::encoded_len(
    "sidecar",
    "attach-fixture",
    &[],
)] = harmos_sidecar::inventory::observers::encode("sidecar", "attach-fixture", &[]);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn main() {
    // `#[used]` only reaches the compiler; MSVC `link.exe` sweeps data no code
    // references, so the declaration is read from live code the way
    // `harmos::plugin::sidecar::run_plugin` reads generated markers.
    std::hint::black_box([
        HARMOS_DECLARATION.as_slice(),
        HARMOS_RESTART.as_slice(),
        MANIFEST.as_slice(),
        PUBLICATIONS.as_slice(),
        OBSERVERS.as_slice(),
    ]);
    assert!(std::env::var_os("HARMOS_ATTACH_HOST_ONLY").is_none());
    assert_eq!(
        std::env::var("HARMOS_ATTACH_EXPLICIT").unwrap(),
        "configured"
    );
    #[cfg(windows)]
    assert!(!std::env::var_os("SystemRoot").unwrap().is_empty());

    let temporary = std::env::temp_dir();
    let probe = temporary.join(format!("harmos-attach-temp-probe-{}", std::process::id()));
    std::fs::write(&probe, b"probe").unwrap();
    std::fs::remove_file(&probe).unwrap();
    #[cfg(windows)]
    {
        let system_root = PathBuf::from(std::env::var_os("SystemRoot").unwrap());
        assert!(!temporary.starts_with(&system_root));
    }
    if let Some(expected) = std::env::var_os("HARMOS_ATTACH_EXPECT_TEMP_DIR") {
        assert_eq!(temporary, PathBuf::from(expected));
    }

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut client =
        TcpStream::connect_timeout(&listener.local_addr().unwrap(), Duration::from_secs(3))
            .unwrap();
    let (mut server, _) = listener.accept().unwrap();
    for socket in [&client, &server] {
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
    }
    client.write_all(b"ping").unwrap();
    let mut payload = [0; 4];
    server.read_exact(&mut payload).unwrap();
    assert_eq!(&payload, b"ping");
    server.write_all(b"pong").unwrap();
    client.read_exact(&mut payload).unwrap();
    assert_eq!(&payload, b"pong");
    println!("networking-ok");
}

/* === Tests === */
