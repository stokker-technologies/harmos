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

use std::{future::Future,pin::Pin};
use harmos_protocol::access::ListenerValue;


// Standard Library Imports

// Third Party Imports

// Local Imports


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type StoreResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub type Pending<'a,T> = Pin<Box<dyn Future<Output=StoreResult<T>> + Send + 'a>>;


/* === Structs === */

/// Complete Atomic Access State. The Host Chooses Its Persistence and Encryption.
#[derive(Clone)]
pub struct Credentials {
 pub revision: u64, pub listener_revision: u64, pub listener: ListenerValue, pub certificate_pem: String, pub private_key_pem: String,
 pub pairings: Vec<Pairing>,
}
#[derive(Clone)]
pub struct Pairing { pub id: String, pub label: String, pub credential_sha256: Vec<u8> }


/* === Enums === */

/* === Traits === */

pub trait CredentialStore: Send + Sync + 'static {
 fn load(&self) -> Pending<'_,Option<Credentials>>;
 /// Compare and Replace Atomically; Zero Requires Absence. Failure Leaves Every Byte Unchanged.
 fn commit(&self,expected_revision:u64,value:Credentials) -> Pending<'_,()>;
}


/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
