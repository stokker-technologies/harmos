/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos::sidecar::inventory::{Declaration, Facility};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("probe_executable_inspectable", || {
            let bytes = std::fs::read(env!("CARGO_BIN_EXE_minibench-probe-sidecar"))
                .expect("the probe executable is readable");
            let declared = Declaration::inspect(&bytes, Facility::Sidecar)
                .expect("the probe declaration is valid");

            assert_eq!(declared.id, "probe-bridge");
            assert_eq!(declared.version.to_string(), env!("CARGO_PKG_VERSION"));
            assert_eq!(
                declared.operations,
                harmos::operations::Declarations::of::<minibench_contract::probe::Operations>()
                    .expect("the portable operation catalog is valid"),
            );
            assert_eq!(
                declared
                    .streams
                    .iter()
                    .map(|stream| (stream.id.as_str(), stream.schema, stream.window))
                    .collect::<Vec<_>>(),
                [("probe-bridge.reading", 1, 32)]
            );
        });
    }
}
