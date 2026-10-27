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
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
// Third Party Imports
use tokio::sync::OwnedSemaphorePermit;
// Local Imports
use super::lane::Admission as LaneAdmission;
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub(super) type Finalize<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/* === Structs === */

pub(super) struct Retained {
    pub(super) owner: Box<dyn Finalizer>,
    pub(super) _permit: Option<OwnedSemaphorePermit>,
    pub(super) _admission: LaneAdmission,
}

/* === Enums === */

/* === Traits === */

pub(super) trait Finalizer: Send {
    fn finalize(&mut self) -> Finalize<'_>;
    fn failure(&self, cause: Arc<Error>) -> Arc<Error> {
        cause
    }
}

/* === Implementations === */

impl crate::supervision::cleanup::Resource for Retained {
    fn finalize(&mut self) -> crate::supervision::cleanup::Finalize<'_> {
        Box::pin(async move { self.owner.finalize().await.map_err(|cause| self.owner.failure(Arc::new(cause))) })
    }
}

/* === Public Functions === */

pub(super) fn failed(operation: &str, outcome: Option<Arc<Error>>, source: Arc<Error>) -> Error {
    Error::OperationFinalization {
        operation: operation.to_owned(),
        outcome,
        source,
    }
}

/* === Private Functions === */

/* === Tests === */
