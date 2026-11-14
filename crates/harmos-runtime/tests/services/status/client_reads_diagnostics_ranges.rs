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
use std::{io::Read, sync::Arc};

// Third Party Imports
use harmos_protocol::host::status as wire;
use sha2::{Digest, Sha256};

// Local Imports
use super::support::*;

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
async fn client_reads_diagnostics_ranges() -> Result {
    // Contribute a Text Entry and Register Its Secret Before Collection
    let host = Host::start().await?;
    let secret = "archive-secret-value";
    host.status.redaction.register(secret.into())?;
    let facts = Arc::new(Facts::new(&format!("host fact: {secret}")));
    let registration = host.status.contribute(facts)?;
    host.status.record_log(wire::Event {
        message: format!("log: {secret}"),
        ..Default::default()
    })?;
    let mut client = wire::diagnostics_client::DiagnosticsClient::new(host.channel.clone());
    // Read One Immutable Archive Through Bounded Unary Ranges
    let first = client
        .get(wire::DiagnosticsGetRequest {
            name: String::new(),
            offset: 0,
            max_bytes: 701,
        })
        .await?
        .into_inner();
    let name = first.name.clone();
    let length = first.total_length;
    let digest = first.sha256.clone();
    let mut bytes = first.data;
    while (bytes.len() as u64) < length {
        let range = client
            .get(wire::DiagnosticsGetRequest {
                name: name.clone(),
                offset: bytes.len() as u64,
                max_bytes: 701,
            })
            .await?
            .into_inner();
        assert_eq!(range.name, name);
        assert_eq!(range.total_length, length);
        assert_eq!(range.sha256, digest);
        assert!(!range.data.is_empty());
        assert!(range.data.len() <= 701);
        bytes.extend(range.data);
    }
    assert_eq!(Sha256::digest(&bytes).as_slice(), digest.as_slice());
    assert!(!String::from_utf8_lossy(&bytes).contains(secret));
    let mut archive = tar::Archive::new(bytes.as_slice());
    let mut contributed = None;
    for entry in archive.entries()? {
        let mut entry = entry?;
        if entry.path()?.as_ref() == std::path::Path::new("host/component.txt") {
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            contributed = Some(text);
        }
    }
    let text = contributed.unwrap();
    assert!(text.contains("host fact:"));
    assert!(!text.contains(secret));
    let end = client
        .get(wire::DiagnosticsGetRequest {
            name: name.clone(),
            offset: length,
            max_bytes: 701,
        })
        .await?
        .into_inner();
    assert!(end.data.is_empty());
    assert_eq!(end.sha256, digest);
    assert!(
        client
            .get(wire::DiagnosticsGetRequest {
                name: name.clone(),
                offset: length + 1,
                max_bytes: 701
            })
            .await
            .is_err()
    );
    assert!(client.get(wire::DiagnosticsGetRequest { name, offset: 0, max_bytes: 4097 }).await.is_err());
    drop((registration, client));
    host.close().await
}
