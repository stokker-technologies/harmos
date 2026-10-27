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
use crate::Result;
use crate::operations::OperationDeclaration;

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
    ) -> Result<Prepared<super::Response<<O as OperationDeclaration>::Response>>>
    + Send
    + Sync;

/* === Structs === */

pub(super) struct Request<T> {
    pub(super) value: T,
    pub(super) retry: Option<super::RetryPolicy>,
    pub(super) lane: Option<&'static str>,
    pub(super) context: Arc<std::sync::OnceLock<super::CallContext>>,
    pub(super) retention: Option<super::streaming::RequestClaims>,
    pub(super) binding: Option<Arc<dyn std::any::Any + Send + Sync>>,
    pub(super) grant: Option<harmos_domain::plugin::Grant>,
    pub(super) key: Option<super::CacheKey>,
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
            retry: None,
            lane: None,
            retention: None,
            binding: None,
            grant: None,
            key: None,
            context: Arc::new(std::sync::OnceLock::from(super::CallContext::default())),
        }
    }
}
impl<T> Request<T> {
    pub(super) fn charged(item: super::StreamItem<T>) -> Result<Self> {
        let (value, claims) = item.into_request()?;
        Ok(Self {
            value,
            retry: None,
            lane: None,
            retention: Some(claims),
            binding: None,
            grant: None,
            key: None,
            context: Arc::new(std::sync::OnceLock::from(super::CallContext::default())),
        })
    }
}

/* === Public Functions === */

pub(super) fn prepared<O>(entry: &Entry, id: OperationInstanceId, body: Box<dyn Body<O>>) -> Prepared<O> {
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

/* === Private Functions === */

/* === Tests === */
