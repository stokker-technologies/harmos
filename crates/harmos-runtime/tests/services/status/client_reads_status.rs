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
use std::sync::Arc;

// Third Party Imports
use harmos::client::v1;

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
async fn client_reads_status() -> Result {
    // Register a Host-Owned Health Component
    let host = Host::start().await?;
    let facts = Arc::new(Facts::new("component diagnostics"));
    let registration = host.status.contribute(facts.clone())?;
    let mut client = v1::status_client::StatusClient::new(host.channel.clone());
    let initial = client.get(v1::StatusGetRequest {}).await?.into_inner();
    let value = initial.status.as_ref().unwrap();
    let identity = value.identity.as_ref().unwrap();
    assert_eq!(identity.application_name, "Example Host");
    assert_eq!(identity.application_version, "1.2.3");
    assert_eq!(identity.library_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(identity.host_id, "stable-example-host");
    assert_eq!(identity.process_started_at.as_ref().unwrap().seconds, 0);
    assert!(value.available);
    assert!(value.components.iter().any(|c| c.name == "host.component" && c.state == v1::ComponentState::Healthy as i32));
    // Observe a Causally Published Health Change
    let mut watch = client.watch(v1::StatusWatchRequest { at: initial.resolved_at }).await?.into_inner();
    let opening = watch.message().await?.unwrap();
    assert!(opening.status.unwrap().available);
    {
        let mut component = facts.component.lock().unwrap();
        component.state = v1::ComponentState::Unavailable as i32;
        component.reason = "Waiting for Host Resource".into();
    }
    registration.changed()?;
    let changed = watch.message().await?.unwrap();
    let status = changed.status.unwrap();
    assert!(!status.available);
    assert!(status.components.iter().any(|c| c.name == "host.component" && c.state == v1::ComponentState::Unavailable as i32));
    // Withdrawing the Registration Removes Its Facts
    drop(registration);
    let removed = watch.message().await?.unwrap().status.unwrap();
    assert!(!removed.components.iter().any(|c| c.name == "host.component"));
    assert!(removed.available);
    drop((watch, client));
    host.close().await
}
