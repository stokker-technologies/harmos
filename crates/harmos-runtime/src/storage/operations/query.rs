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
use crate::operations::{Context, Operation, OperationDeclaration, StreamOutput};
use crate::storage::StorageManager;
use super::IO_TIMEOUT;
use crate::storage::query::{QueryCompletion, QueryOutput};
use crate::{Error, MemorySize, Result};
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
    type Declaration = Query;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<QueryOutput>;
    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }
    async fn run<'a>(
        &mut self,
        ctx: Self::Context<'a>,
        request: &mut Query,
        output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<QueryCompletion>> {
        let grant = Grant::of(Query::ID, request.source.grant_scope());
        ctx.admit_stream_grant(&grant)?;
        let mut reader = self.storage.query(request.clone()).await?;
        while let Some(item) = reader.next().await? {
            ctx.admit_stream_grant(&grant)?;
            let item = item.output();
            tokio::time::timeout(
                IO_TIMEOUT,
                output.send_with(item.memory_size(), || Ok(item)),
            )
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
