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

use harmos_domain::operations::storage::StorageStatus;

// Standard Library Imports

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::Operation;
use crate::storage::StorageInfo;

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
    type Request = <StorageStatus as harmos_domain::OperationDeclaration>::Request;
    type Response = <StorageStatus as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = ();
    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<StorageManager<C>>()?;
        Ok(Self { storage })
    }

    async fn run(
        &mut self, _: harmos_sdk::OperationContext, _request: &mut Self::Request, _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageInfo>> {
        let result: ::core::result::Result<StorageInfo, Self::Error> = async move { self.storage.info().map_err(Into::into) }.await;
        result.map(::core::convert::Into::into)
    }

    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
