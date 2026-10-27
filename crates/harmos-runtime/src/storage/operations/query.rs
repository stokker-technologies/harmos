/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Finite Storage Query Operation

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::IO_TIMEOUT;
use crate::operations::{Operation, OperationDeclaration, StreamOutput};
use crate::storage::StorageManager;
use crate::storage::query::{QueryCompletion, QueryOutput};
use crate::{Error, Result};
use harmos_domain::operations::storage::Query;
use harmos_domain::plugin::Grant;
use std::sync::Arc;

// Standard Library Imports

// Third Party Imports

// Local Imports

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
    type Request = <Query as harmos_domain::OperationDeclaration>::Request;
    type Response = <Query as harmos_domain::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = StreamOutput<QueryOutput>;
    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<StorageManager<C>>()?;
        Ok(Self { storage })
    }
    async fn run(
        &mut self, ctx: harmos_sdk::OperationContext, request: &mut Self::Request, output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<QueryCompletion>> {
        let host = ctx.dependency::<crate::operations::HostContext>()?;
        let ctx = host.borrow();
        let grant = Grant::of(Query::ID, request.source.grant_scope());
        ctx.admit_stream_grant(&grant)?;
        let mut reader = self
            .storage
            .query(harmos_storage::Query {
                at: request.at.clone(),
                source: request.source.clone(),
                transforms: request.transforms.clone(),
                slice: request.slice,
            })
            .await?;
        while let Some(item) = reader.next().await? {
            ctx.admit_stream_grant(&grant)?;
            let bytes = item.output_memory_size();
            tokio::time::timeout(IO_TIMEOUT, output.send_with(bytes, || Ok(item.output())))
                .await
                .map_err(|_| super::output_timeout())??;
        }
        Ok(reader.completion()?.into())
    }
    fn classify(error: &Error) -> crate::operations::FailureKind {
        super::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
