/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */




/* === Modules === */

mod listener_get;
mod listener_set;
mod pairings_create;
mod pairings_list;
mod pairings_delete;
mod certificates_create;


/* === Re-Exports === */

pub use listener_get::ListenerGet;
pub use listener_set::ListenerSet;
pub use pairings_create::PairingsCreate;
pub use pairings_list::PairingsList;
pub use pairings_delete::PairingsDelete;
pub use certificates_create::CertificatesCreate;


/* === Imports === */

use std::{future::Future, pin::Pin, time::Duration};
use crate::execution::operations::ServingError;


// Standard Library Imports

// Third Party Imports

// Local Imports


/* === Constants === */

const POLICY: crate::OperationPolicy = crate::OperationPolicy {
 timeout: crate::operations::UNBOUNDED, attempt_timeout: crate::operations::UNBOUNDED,
 cancellation_timeout: Duration::ZERO, retries: crate::RetryPolicy::Never,
};


/* === Macros === */

/* === Type Aliases === */

pub type Pending<'a, T> = Pin<Box<dyn Future<Output = Result<T, ServingError>> + Send + 'a>>;


/* === Structs === */

#[derive(Default)]
pub struct AccessAccess(pub std::sync::RwLock<Option<std::sync::Weak<dyn Access>>>);


/* === Enums === */

/* === Traits === */

pub trait Access: Send + Sync {
    fn listener_get(&self, request: harmos_protocol::access::ListenerGetRequest) -> Pending<'_, harmos_protocol::access::ListenerGetResponse>;
    fn listener_set(&self, request: harmos_protocol::access::ListenerSetRequest) -> Pending<'_, harmos_protocol::access::ListenerSetResponse>;
    fn pairings_create(&self, request: harmos_protocol::access::PairingsCreateRequest) -> Pending<'_, harmos_protocol::access::PairingsCreateResponse>;
    fn pairings_list(&self, request: harmos_protocol::access::PairingsListRequest) -> Pending<'_, harmos_protocol::access::PairingsListResponse>;
    fn pairings_delete(&self, request: harmos_protocol::access::PairingsDeleteRequest) -> Pending<'_, harmos_protocol::access::PairingsDeleteResponse>;
    fn certificates_create(&self, request: harmos_protocol::access::CertificatesCreateRequest) -> Pending<'_, harmos_protocol::access::CertificatesCreateResponse>;
}


/* === Implementations === */

/* === Public Functions === */

pub fn catalog() -> Result<crate::execution::__private::Catalog, ServingError> {
 use crate::execution::operations::OperationFactory;
 let catalog = crate::execution::__private::Catalog::new();
 let catalog = catalog.entry(OperationFactory::protobuf::<ListenerGet>()?);
 let catalog = catalog.entry(OperationFactory::protobuf::<ListenerSet>()?);
 let catalog = catalog.entry(OperationFactory::protobuf::<PairingsCreate>()?);
 let catalog = catalog.entry(OperationFactory::protobuf::<PairingsList>()?);
 let catalog = catalog.entry(OperationFactory::protobuf::<PairingsDelete>()?);
 let catalog = catalog.entry(OperationFactory::protobuf::<CertificatesCreate>()?);
 Ok(catalog)
}


/* === Private Functions === */

/* === Tests === */
