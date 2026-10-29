/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// An observer's instance opens by awaiting whatever it opens, so a synchronous
// `initialize` is refused where it is written rather than at the trait boundary.


pub struct Level;


#[harmos::observer(interval_ms = 100)]
impl Level {
    fn initialize(_resources: &harmos::ExecutionContext) -> Result<Self, String> {
        Ok(Self)
    }

    async fn observe(&mut self, _context: &harmos::sidecar::ObserverContext<'_>) -> Result<(), String> {
        Ok(())
    }
}


fn main() {}
