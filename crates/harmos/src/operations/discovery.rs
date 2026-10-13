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
use super::host::{Operations, OperationsRequest, OperationsResponse};
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

    fn new(_: Arc<()>) -> Result<Self> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut OperationsRequest,
        _: &mut Self::Endpoints,
    ) -> Result<OperationsResponse> {
        context.operations.discover(&context.source, request)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
