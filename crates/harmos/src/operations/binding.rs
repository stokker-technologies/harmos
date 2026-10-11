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
use tokio::sync::OwnedSemaphorePermit;
// Local Imports
use super::OperationInstanceId;
use super::admission::Admission;
use super::caller::{Origin, Source};
use super::execution::{Body, Prepared};
use super::manager::Entry;
use crate::operations::{Failure, OperationDeclaration};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Prepare<O, P> = dyn Fn(
        (Request<<O as OperationDeclaration>::Request>, P),
        OperationInstanceId,
        Arc<crate::supervision::instance::Instance>,
        Arc<Entry>,
        Admission,
        Source,
        Origin,
        Option<OwnedSemaphorePermit>,
    ) -> Result<Prepared<Arc<<O as OperationDeclaration>::Response>>>
    + Send
    + Sync;

/* === Structs === */
pub(super) struct Request<T> {
    pub(super) value: T,
    pub(super) context: Arc<std::sync::OnceLock<super::CallContext>>,
    pub(super) retention: Option<super::streaming::RequestClaims>,
}

pub(super) struct Typed<O: OperationDeclaration, P = ()> {
    pub(super) prepare: Box<Prepare<O, P>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<T> From<T> for Request<T> {
    fn from(value: T) -> Self {
        Self {
            value,
            retention: None,
            context: Arc::new(std::sync::OnceLock::from(super::CallContext::default())),
        }
    }
}
impl<T> Request<T> {
    pub(super) fn charged(item: super::StreamItem<T>) -> Self {
        let (value, claims) = item.into_request();
        Self {
            value,
            retention: Some(claims),
            context: Arc::new(std::sync::OnceLock::from(super::CallContext::default())),
        }
    }
}

/* === Public Functions === */
pub(super) fn prepared<O>(
    entry: &Entry,
    id: OperationInstanceId,
    body: Box<dyn Body<O>>,
) -> Prepared<O> {
    Prepared {
        body,
        policy: entry.declared().policy,
        replay: entry.declared().replay,
        id,
        completed: None,
        refusal: None,
        middleware: None,
    }
}

pub(super) fn classify<E: std::error::Error + Send + Sync + 'static>(
    error: &Error,
    classify: fn(&E) -> Failure,
) -> Failure {
    let Error::OperationFailed { source, .. } = error else {
        return Failure::Permanent;
    };
    source
        .downcast_ref::<E>()
        .map_or(Failure::Permanent, |error| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| classify(error)))
                .unwrap_or(Failure::Permanent)
        })
}

/* === Private Functions === */
/* === Tests === */
