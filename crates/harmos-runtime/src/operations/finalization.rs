/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::OwnedSemaphorePermit;

use super::lane::Admission as LaneAdmission;
use crate::{Error, Result};

pub(super) type Finalize<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

pub(super) struct Retained {
    pub(super) owner: Box<dyn Finalizer>,
    pub(super) _permit: Option<OwnedSemaphorePermit>,
    pub(super) _admission: LaneAdmission,
}

pub(super) trait Finalizer: Send {
    fn finalize(&mut self) -> Finalize<'_>;
    fn failure(&self, cause: Arc<Error>) -> Arc<Error> {
        cause
    }
}

impl crate::supervision::cleanup::Resource for Retained {
    fn finalize(&mut self) -> crate::supervision::cleanup::Finalize<'_> {
        Box::pin(async move { self.owner.finalize().await.map_err(|cause| self.owner.failure(Arc::new(cause))) })
    }
}

pub(super) fn failed(operation: &str, outcome: Option<Arc<Error>>, source: Arc<Error>) -> Error {
    Error::OperationFinalization {
        operation: operation.to_owned(),
        outcome,
        source,
    }
}
