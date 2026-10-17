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

// Local Imports
use super::host::{Operations, OperationsResponse};
use super::{Context, Operation};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// Explicitly Register This Handler In The Original Host Operation Manager.
pub struct Discovery;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Operation for Discovery {
    type Declaration = Operations;
    type Resources = ();
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut Operations,
        _: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<OperationsResponse>> {
        let result: ::core::result::Result<OperationsResponse, Self::Error> =
            async move { context.operations.discover(&context.source, request) }.await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
