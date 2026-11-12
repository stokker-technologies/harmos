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
use harmos_protocol::access;
use std::sync::atomic::Ordering;


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
async fn credential_store_unavailable()->Result{
 // Establish Access Before the Store Becomes Unavailable
 let host=Host::start().await?;host.enable().await?;
 let mut listener=access::listener_client::ListenerClient::new(host.channel.clone());let mut pairings=access::pairings_client::PairingsClient::new(host.channel.clone());let mut certificates=access::certificates_client::CertificatesClient::new(host.channel.clone());
 let original=listener.get(access::ListenerGetRequest{}).await?.into_inner();let before=host.store.value.lock().unwrap().as_ref().unwrap().clone();
 host.store.unavailable.store(true,Ordering::Release);
 assert!(listener.get(access::ListenerGetRequest{}).await.unwrap_err().message().contains("Access.Listener.Get"));
 assert!(pairings.create(access::PairingsCreateRequest{label:"Unavailable".into()}).await.unwrap_err().message().contains("Access.Pairings.Create"));
 assert!(pairings.list(access::PairingsListRequest{page:Some(page(1,String::new())),at:None}).await.unwrap_err().message().contains("Access.Pairings.List"));
 assert!(pairings.delete(access::PairingsDeleteRequest{name:"missing".into()}).await.unwrap_err().message().contains("Access.Pairings.Delete"));
 assert!(certificates.create(access::CertificatesCreateRequest{}).await.unwrap_err().message().contains("Access.Certificates.Create"));
 let mut value=original.value.clone().unwrap();value.enabled=false;
 assert!(listener.set(access::ListenerSetRequest{value:Some(value.clone()),expected_revision:original.revision.clone()}).await.unwrap_err().message().contains("Access.Listener.Set"));
 // Fail an Atomic Commit After a Successful Load
 host.store.unavailable.store(false,Ordering::Release);host.store.fail_commit.store(true,Ordering::Release);
 assert!(listener.set(access::ListenerSetRequest{value:Some(value),expected_revision:original.revision.clone()}).await.is_err());
 assert!(certificates.create(access::CertificatesCreateRequest{}).await.is_err());
 assert!(pairings.create(access::PairingsCreateRequest{label:"Commit Fails".into()}).await.is_err());
 host.store.fail_commit.store(false,Ordering::Release);
 assert_eq!(listener.get(access::ListenerGetRequest{}).await?.into_inner(),original);
 let after=host.store.value.lock().unwrap().as_ref().unwrap().clone();assert_eq!(after.revision,before.revision);assert_eq!(after.certificate_pem,before.certificate_pem);assert_eq!(after.private_key_pem,before.private_key_pem);assert!(after.pairings.is_empty());
 // The Same Live Listener Still Accepts a New Pairing
 let pairing=pairings.create(access::PairingsCreateRequest{label:"Recovered".into()}).await?.into_inner();assert_eq!(pairing.pinned_certificate_pem,before.certificate_pem);let connected=paired(&pairing).await?;
 drop((connected,listener,pairings,certificates));host.close().await
}

