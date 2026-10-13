/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![cfg(feature = "guest")]
/* === Modules === */
mod operations;
mod streams;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Guest;

/* === Enums === */

/* === Traits === */
trait ProductPlugin {}

/* === Implementations === */
impl harmos::operations::lifecycle::Policy for Guest {
    const INITIALIZATION_POLICY: harmos::operations::OperationPolicy = operations::SDK_POLICY;
    const SHUTDOWN_POLICY: harmos::operations::OperationPolicy = operations::SDK_POLICY;
}

#[harmos::plugin(description = "Quoted \"probe\" line \\ \u{3bc}\u{1}", id = "cargo-guest", context = harmos::Context, config = String,
    provides = self::operations::Operations, requires = (), streams(self::streams::Reading))]
impl ProductPlugin for Guest {}

/* === Public Functions === */
#[unsafe(no_mangle)]
pub extern "C" fn anchor() -> usize {
    let markers = std::hint::black_box(<Guest as harmos::plugin::Declared>::markers().unwrap());
    markers.iter().map(|marker| marker.len()).sum()
}

/* === Private Functions === */

/* === Tests === */
