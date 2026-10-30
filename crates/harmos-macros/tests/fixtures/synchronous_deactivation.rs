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


pub struct Level;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[harmos::observer]
impl Level {
    fn interval(_: &harmos::ExecutionContext) -> Result<std::time::Duration, String> {
        Ok(std::time::Duration::from_millis(100))
    }

    async fn initialize(_resources: &harmos::ExecutionContext) -> Result<Self, String> {
        Ok(Self)
    }

    async fn observe(&mut self, _context: &harmos::sidecar::ObserverContext<'_>) -> Result<(), String> {
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> { Ok(()) }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
