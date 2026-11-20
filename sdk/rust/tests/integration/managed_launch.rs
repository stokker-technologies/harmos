/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Launch Announcement

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{net::SocketAddr, process::Stdio};

// Third Party Imports
use tokio::process::Command;

// Local Imports
use crate::support::{Bootstrap, binary};
use harmos_protocol::plugin::{InstanceStopRequest, instance_client::InstanceClient};

/* === Constants === */

const PLUGIN: &str = "harmos-sdk-host-fixture";

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
async fn managed_run_announces_once_and_writes_nothing() {
    let cleanup = tempfile::tempdir().unwrap();
    let mut bootstrap = Bootstrap::start(None).await.unwrap();
    let child = bootstrap
        .command(PLUGIN)
        .arg(cleanup.path().join("stopped"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let announce = bootstrap.announced().await.unwrap();
    assert_eq!(announce.token, Bootstrap::TOKEN);
    let address: SocketAddr = announce.address.parse().unwrap();
    assert!(address.ip().is_loopback());
    assert_ne!(address.port(), 0);
    let mut client = InstanceClient::connect(format!("http://{address}")).await.unwrap();
    client.stop(InstanceStopRequest { reason: "Story Stop".into() }).await.unwrap();
    let output = child.wait_with_output().await.unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(bootstrap.pending(), 0);
}

#[tokio::test]
async fn refused_announce_fails_the_start_with_the_host_message() {
    let cleanup = tempfile::tempdir().unwrap();
    let bootstrap = Bootstrap::start(Some("Launch Credential Is Invalid")).await.unwrap();
    let output = bootstrap
        .command(PLUGIN)
        .arg(cleanup.path().join("stopped"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Launch Credential Is Invalid"));
}

#[tokio::test]
async fn one_launch_variable_is_a_start_error_naming_the_other() {
    let cleanup = tempfile::tempdir().unwrap();
    for (present, missing) in [("HARMOS_HOST_ENDPOINT", "HARMOS_LAUNCH_TOKEN"), ("HARMOS_LAUNCH_TOKEN", "HARMOS_HOST_ENDPOINT")] {
        let output = Command::new(binary(PLUGIN))
            .arg(cleanup.path().join("stopped"))
            .env_remove(missing)
            .env(present, "127.0.0.1:9")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output()
            .await
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(&format!("Set {missing}")));
    }
}
