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
use std::sync::{Arc, Mutex};

// Third Party Imports
use harmos_protocol::client::access;
use harmos_protocol::v1::{self};

// Local Imports
use super::support::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct AccessPolicy(Arc<Mutex<Vec<String>>>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::operations::authorization::Authorizer for AccessPolicy {
    fn authorize(
        &self, context: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        if let Some(principal) = context.get::<harmos_runtime::grpc::access::Principal>() {
            self.0.lock().unwrap().push(principal.pairing_id.clone());
        }
        Ok(Some(harmos_runtime::operations::authorization::Authorization::Full(provider.contract.clone())))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn client_pairs_and_rotates_access() -> Result {
    // Read and Replace the Whole Listener Value
    let host = Host::start().await?;
    let principals = Arc::new(Mutex::new(Vec::new()));
    host.runtime.operations.register_authorizer(Arc::new(AccessPolicy(principals.clone())))?;
    let mut listener = access::listener_client::ListenerClient::new(host.channel.clone());
    let original = listener.get(access::ListenerGetRequest {}).await?.into_inner();
    assert!(!original.value.as_ref().unwrap().enabled);
    let enabled = host.enable().await?;
    assert!(enabled.bound_endpoint.starts_with("https://"));
    assert!(
        listener
            .set(access::ListenerSetRequest {
                value: original.value,
                expected_revision: original.revision
            })
            .await
            .is_err()
    );
    // Pair a Second Client Using the Pinned Certificate
    let mut pairings = access::pairings_client::PairingsClient::new(host.channel.clone());
    let first = pairings.create(access::PairingsCreateRequest { label: "First Client".into() }).await?.into_inner();
    let channel = paired(&first).await?;
    let mut client = harmos_protocol::client::status_client::StatusClient::new(channel.clone());
    assert_eq!(client.get(harmos_protocol::client::StatusGetRequest {}).await.unwrap_err().code(), tonic::Code::Unauthenticated);
    let identity = client
        .get(authorized(harmos_protocol::client::StatusGetRequest {}, &first.credential))
        .await?
        .into_inner()
        .status
        .unwrap()
        .identity
        .unwrap();
    assert!(principals.lock().unwrap().contains(&first.pairing.as_ref().unwrap().id));
    // Rotate Live Trust and Refuse the Old Credential on an Existing Connection
    let mut certificates = access::certificates_client::CertificatesClient::new(host.channel.clone());
    let rotated = certificates.create(access::CertificatesCreateRequest {}).await?.into_inner();
    assert_ne!(rotated.certificate_pem, first.pinned_certificate_pem);
    assert_eq!(
        client.get(authorized(harmos_protocol::client::StatusGetRequest {}, &first.credential)).await.unwrap_err().code(),
        tonic::Code::Unauthenticated
    );
    assert!(paired(&first).await.is_err());
    let second = pairings.create(access::PairingsCreateRequest { label: "Current Client".into() }).await?.into_inner();
    let mut current = harmos_protocol::client::status_client::StatusClient::new(paired(&second).await?);
    let after = current
        .get(authorized(harmos_protocol::client::StatusGetRequest {}, &second.credential))
        .await?
        .into_inner()
        .status
        .unwrap()
        .identity
        .unwrap();
    assert_eq!(identity, after);
    assert_eq!(listener.get(access::ListenerGetRequest {}).await?.into_inner().revision, enabled.revision);
    // List and Revoke a Durable Pairing Without Exposing Its Credential
    let page = pairings.list(access::PairingsListRequest { at: None }).await?.into_inner();
    assert_eq!(page.pairings.len(), 1);
    assert_eq!(page.pairings[0].id, second.pairing.as_ref().unwrap().id);
    let store = host.store.clone();
    drop((client, channel, current, listener, pairings, certificates));
    host.close().await?;
    // Restart Retains Credentials and Trust Under the Same Host Identity
    let reopened = Host::with_store(store).await?;
    let bound =
        access::listener_client::ListenerClient::new(reopened.channel.clone()).get(access::ListenerGetRequest {}).await?.into_inner();
    let mut retained = second.clone();
    retained.endpoint = bound.bound_endpoint;
    let mut current = harmos_protocol::client::status_client::StatusClient::new(paired(&retained).await?);
    assert_eq!(
        current
            .get(authorized(harmos_protocol::client::StatusGetRequest {}, &retained.credential))
            .await?
            .into_inner()
            .status
            .unwrap()
            .identity
            .unwrap()
            .host_id,
        identity.host_id
    );
    let mut pairings = access::pairings_client::PairingsClient::new(reopened.channel.clone());
    pairings.delete(access::PairingsDeleteRequest { name: retained.pairing.unwrap().id }).await?;
    assert_eq!(
        current.get(authorized(harmos_protocol::client::StatusGetRequest {}, &retained.credential)).await.unwrap_err().code(),
        tonic::Code::Unauthenticated
    );
    drop((current, pairings));
    reopened.close().await
}
