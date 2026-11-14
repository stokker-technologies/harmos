/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod certificates_create;
mod listener_get;
mod listener_set;
mod pairings_create;
mod pairings_delete;
mod pairings_list;

/* === Re-Exports === */

pub use certificates_create::CertificatesCreate;
pub use listener_get::ListenerGet;
pub use listener_set::ListenerSet;
pub use pairings_create::PairingsCreate;
pub use pairings_delete::PairingsDelete;
pub use pairings_list::PairingsList;

/* === Imports === */

use harmos_protocol::host::access::CertificatesCreateRequest;
use harmos_protocol::host::access::CertificatesCreateResponse;
use harmos_protocol::host::access::ListenerGetRequest;
use harmos_protocol::host::access::ListenerGetResponse;
use harmos_protocol::host::access::ListenerSetRequest;
use harmos_protocol::host::access::ListenerSetResponse;
use harmos_protocol::host::access::PairingsCreateRequest;
use harmos_protocol::host::access::PairingsCreateResponse;
use harmos_protocol::host::access::PairingsDeleteRequest;
use harmos_protocol::host::access::PairingsDeleteResponse;
use harmos_protocol::host::access::PairingsListRequest;
use harmos_protocol::host::access::PairingsListResponse;

// Standard Library Imports
use std::{future::Future, pin::Pin, time::Duration};

// Third Party Imports

// Local Imports
use crate::execution::operations::ServingError;

/* === Constants === */

const POLICY: crate::OperationPolicy = crate::OperationPolicy {
    timeout: crate::operations::UNBOUNDED,
    attempt_timeout: crate::operations::UNBOUNDED,
    cancellation_timeout: Duration::ZERO,
    retries: crate::RetryPolicy::Never,
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
    fn listener_get(&self, request: ListenerGetRequest) -> Pending<'_, ListenerGetResponse>;
    fn listener_set(&self, request: ListenerSetRequest) -> Pending<'_, ListenerSetResponse>;
    fn pairings_create(&self, request: PairingsCreateRequest) -> Pending<'_, PairingsCreateResponse>;
    fn pairings_list(&self, request: PairingsListRequest) -> Pending<'_, PairingsListResponse>;
    fn pairings_delete(&self, request: PairingsDeleteRequest) -> Pending<'_, PairingsDeleteResponse>;
    fn certificates_create(&self, request: CertificatesCreateRequest) -> Pending<'_, CertificatesCreateResponse>;
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
