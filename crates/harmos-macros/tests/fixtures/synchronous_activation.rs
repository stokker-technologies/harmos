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


pub struct Level;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[harmos::observer]
impl Level {
    fn interval(_: &harmos::ExecutionContext) -> Result<std::time::Duration, String> {
        Ok(std::time::Duration::from_millis(100))
    }

    fn initialize(_resources: &harmos::ExecutionContext) -> Result<Self, String> {
        Ok(Self)
    }

    async fn observe(&mut self, _context: &harmos::sidecar::ObserverContext<'_>) -> Result<(), String> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
