/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Bind Greeting Behavior To The Original SDK Runtime.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
use std::sync::Arc;
use crate::domain::operations::greeting::Greet;


// Standard Library Imports

// Third Party Imports

// Local Imports


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Greeting;


/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos_sidecar::operation]
impl Greeting {
    async fn initialize(_: Arc<()>, _: &harmos_sidecar::Supervisor) -> Self { Self }
    async fn run(&mut self, _: &harmos_sidecar::Supervisor, request: &mut Greet) -> Result<crate::domain::model::greeting::Greeting, std::io::Error> {
        Ok(crate::domain::model::greeting::Greeting::for_name(&request.name))
    }
}


/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
