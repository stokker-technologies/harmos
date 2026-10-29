/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// The mirror of synchronous initialization: an instance releases what it holds by
// awaiting it, so a synchronous `stop` is refused where it is written.


pub struct Level;


#[harmos::observer(interval_ms = 100)]
impl Level {
    async fn initialize(_resources: &harmos::ExecutionContext) -> Result<Self, String> {
        Ok(Self)
    }

    async fn observe(&mut self, _context: &harmos::sidecar::ObserverContext<'_>) -> Result<(), String> {
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> { Ok(()) }
}


fn main() {}
