/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// HashFile Runtime Handler.

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
use harmos_domain::model::storage::files::FileHash;
use harmos_domain::operations::storage::HashFile;

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
    type Declaration = HashFile;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    async fn initialize<'ctx>(storage: Arc<StorageManager<C>>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(&mut self, _: Self::Context<'a>, request: &mut HashFile, _: &mut ()) -> Result<crate::operations::Response<FileHash>> {
        Ok(self.storage.hash_file(request.clone()).await?.into())
    }

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
