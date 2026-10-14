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
// An observer's instance opens by awaiting whatever it opens, so a synchronous
// `initialize` is refused where it is written rather than at the trait boundary.
pub struct Resources;

pub struct Level;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos::observer(interval_ms = 100)]
impl Level {
    fn initialize(_resources: &Resources) -> Result<Self, String> {
        Ok(Self)
    }

    async fn observe(&mut self, _context: &harmos::sidecar::ObserverContext<'_, Resources>) -> Result<(), String> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn main() {}

/* === Tests === */
