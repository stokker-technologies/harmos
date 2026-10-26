/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// CaptureFile Runtime Handler.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::operations::{Context, Operation};
use crate::storage::StorageManager;
use crate::{Error, Result};
use harmos_domain::model::storage::files::FileReference;
use harmos_domain::operations::storage::CaptureFile;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C: 'static> Operation for Handler<C> {
    type Declaration = CaptureFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    async fn initialize<'ctx>(storage: Arc<StorageManager<C>>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self { storage })
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.operation.capture_file", level = "trace", skip_all)]
    async fn run<'a>(
        &mut self, _: Self::Context<'a>, request: &mut CaptureFile, _: &mut (),
    ) -> Result<crate::operations::Response<FileReference>> {
        Ok(self.storage.capture_file(request.clone()).await?.into())
    }

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
