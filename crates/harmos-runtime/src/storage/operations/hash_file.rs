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
use crate::operations::Operation;
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
    type Request = <HashFile as harmos_domain::OperationDeclaration>::Request;
    type Response = <HashFile as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = ();

    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<StorageManager<C>>()?;
        Ok(Self { storage })
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.operation.hash_file", level = "trace", skip_all)]
    async fn run(
        &mut self, _: harmos_sdk::OperationContext, request: &mut Self::Request, _: &mut (),
    ) -> Result<crate::operations::Response<FileHash>> {
        Ok(self.storage.hash_file(request.clone()).await?.into())
    }

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
