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

use super::support::*;
use harmos_protocol::{access,v1};


// Standard Library Imports

// Third Party Imports

// Local Imports


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
async fn client_pairs_and_rotates_access()->Result{
 // Read and Replace the Whole Listener Value
 let host=Host::start().await?;let mut listener=access::listener_client::ListenerClient::new(host.channel.clone());
 let original=listener.get(access::ListenerGetRequest{}).await?.into_inner();assert!(!original.value.as_ref().unwrap().enabled);
 let enabled=host.enable().await?;assert!(enabled.bound_endpoint.starts_with("https://"));
 assert!(listener.set(access::ListenerSetRequest{value:original.value,expected_revision:original.revision}).await.is_err());
 // Pair a Second Client Using the Pinned Certificate
 let mut pairings=access::pairings_client::PairingsClient::new(host.channel.clone());
 let first=pairings.create(access::PairingsCreateRequest{label:"First Client".into()}).await?.into_inner();
 let channel=paired(&first).await?;let mut client=v1::status_client::StatusClient::new(channel.clone());
 assert_eq!(client.get(v1::StatusGetRequest{}).await.unwrap_err().code(),tonic::Code::Unauthenticated);
 let identity=client.get(authorized(v1::StatusGetRequest{},&first.credential)).await?.into_inner().status.unwrap().identity.unwrap();
 // Rotate Live Trust and Refuse the Old Credential on an Existing Connection
 let mut certificates=access::certificates_client::CertificatesClient::new(host.channel.clone());
 let rotated=certificates.create(access::CertificatesCreateRequest{}).await?.into_inner();assert_ne!(rotated.certificate_pem,first.pinned_certificate_pem);
 assert_eq!(client.get(authorized(v1::StatusGetRequest{},&first.credential)).await.unwrap_err().code(),tonic::Code::Unauthenticated);
 assert!(paired(&first).await.is_err());
 let second=pairings.create(access::PairingsCreateRequest{label:"Current Client".into()}).await?.into_inner();
 let mut current=v1::status_client::StatusClient::new(paired(&second).await?);
 let after=current.get(authorized(v1::StatusGetRequest{},&second.credential)).await?.into_inner().status.unwrap().identity.unwrap();assert_eq!(identity,after);
 // List and Revoke a Durable Pairing Without Exposing Its Credential
 let page=pairings.list(access::PairingsListRequest{page:Some(page(1,String::new())),at:None}).await?.into_inner();assert_eq!(page.pairings.len(),1);assert_eq!(page.pairings[0].id,second.pairing.as_ref().unwrap().id);
 let store=host.store.clone();drop((client,channel,current,listener,pairings,certificates));host.close().await?;
 // Restart Retains Credentials and Trust Under the Same Host Identity
 let reopened=Host::with_store(store).await?;
 let bound=access::listener_client::ListenerClient::new(reopened.channel.clone()).get(access::ListenerGetRequest{}).await?.into_inner();
 let mut retained=second.clone();retained.endpoint=bound.bound_endpoint;
 let mut current=v1::status_client::StatusClient::new(paired(&retained).await?);
 assert_eq!(current.get(authorized(v1::StatusGetRequest{},&retained.credential)).await?.into_inner().status.unwrap().identity.unwrap().host_id,identity.host_id);
 let mut pairings=access::pairings_client::PairingsClient::new(reopened.channel.clone());pairings.delete(access::PairingsDeleteRequest{name:retained.pairing.unwrap().id}).await?;
 assert_eq!(current.get(authorized(v1::StatusGetRequest{},&retained.credential)).await.unwrap_err().code(),tonic::Code::Unauthenticated);
 drop((current,pairings));reopened.close().await
}

