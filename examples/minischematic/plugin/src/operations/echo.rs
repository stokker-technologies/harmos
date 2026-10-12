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
use std::convert::Infallible;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use minischematic_contract::autoroute::{Echo, EchoRequest, EchoResponse};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct EchoOperation;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos::operation(declaration = Echo)]
impl EchoOperation {
    fn new(_: Arc<()>) -> Self {
        Self
    }

    async fn run(
        &mut self,
        _context: &harmos::guest::Context,
        request: &mut EchoRequest,
    ) -> Result<EchoResponse, Infallible> {
        Ok(EchoResponse {
            text: request.text.clone(),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
