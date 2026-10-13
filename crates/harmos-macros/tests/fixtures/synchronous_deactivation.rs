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

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// The mirror of synchronous initialization: an instance releases what it holds by
// awaiting it, so a synchronous `stop` is refused where it is written.
pub struct Resources;

pub struct Level;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos::observer(interval_ms = 100)]
impl Level {
    async fn initialize(_resources: &Resources) -> Result<Self, String> {
        Ok(Self)
    }

    async fn observe(&mut self, _context: &harmos::plugin::sidecar::ObserverContext<'_, Resources>) -> Result<(), String> {
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> { Ok(()) }
}

/* === Public Functions === */

/* === Private Functions === */
fn main() {}

/* === Tests === */
