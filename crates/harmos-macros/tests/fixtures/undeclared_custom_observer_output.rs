/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


/* === Modules === */
#[path = "policy.rs"]
mod controls;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Bridge;

struct Host;

#[derive(Default)]
struct Watch;

#[harmos::stream(id = "undeclared.output", window = 4)]
struct Output { value: u64 }

/* === Enums === */
#[harmos::catalog]
enum Observers { Watch(Watch) }

/* === Traits === */
trait ProductPlugin {}

/* === Implementations === */
impl harmos::sidecar::ObserverHost<()> for Host {
    fn bind(_: std::sync::Arc<()>, _: harmos::sidecar::Supervisor) -> Self { Self }
}

impl harmos::operations::lifecycle::Policy for Bridge {
    const INITIALIZATION_POLICY: harmos::operations::OperationPolicy = controls::POLICY;
    const SHUTDOWN_POLICY: harmos::operations::OperationPolicy = controls::POLICY;
}

#[harmos::observer(interval_ms = 5, streams(Output))]
impl Watch {
    async fn observe(&mut self, _: &harmos::sidecar::ObserverContext<'_, (), Host>) -> Result<(), harmos::sidecar::Refusal> { Ok(()) }
}

#[harmos::plugin(id = "missing-output", context = harmos::sidecar::Supervisor, config = (),
    provides = (), requires = (), observers = Observers, restart = "never")]
impl ProductPlugin for Bridge {}

/* === Public Functions === */

/* === Private Functions === */
fn main() {}

/* === Tests === */
