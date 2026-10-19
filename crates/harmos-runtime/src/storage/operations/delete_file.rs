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

use harmos_domain::operations::storage::DeleteFile;

// Standard Library Imports

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::{Context, Operation};

use crate::storage::StagedFile;
use crate::storage::StorageManager;

use crate::{Error, Result};

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
    type Declaration = DeleteFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    async fn initialize<'ctx>(storage: Arc<StorageManager<C>>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self, _: Self::Context<'a>, request: &mut DeleteFile, _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StagedFile>> {
        let result: ::core::result::Result<StagedFile, Self::Error> =
            async move { self.storage.delete_file_once(&request.mutation, &request.file).await.map_err(Into::into) }.await;
        result.map(::core::convert::Into::into)
    }

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
