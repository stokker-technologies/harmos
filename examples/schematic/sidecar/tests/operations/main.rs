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
use harmos::plugin::Declared;
use harmos::sidecar::inventory::{Declaration, Facility};
use harmos::sidecar::operations::{Declarations, OperationDeclaration};
use schematic_domain::operations::{ListDirectories, Operations};

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
    scenario_runtime::check("schematic_executable_inspectable", || {
        let bytes = std::fs::read(env!("CARGO_BIN_EXE_schematic-sidecar")).unwrap();
        let declaration = Declaration::inspect(&bytes, Facility::Sidecar).unwrap();
        assert_eq!(declaration.id, "schematic-tools");
        assert_eq!(declaration.version.to_string(), env!("CARGO_PKG_VERSION"));
        let expected = Declarations::of::<schematic_domain::operations::Native>().unwrap();
        assert_eq!(declaration.operations, expected);
        assert!(declaration.streams.is_empty());
    });
    scenario_runtime::check("guest_omits_native_filesystem", || {
        let shared = Declarations::of::<Operations>().unwrap();
        assert_eq!(shared.len(), 3);
        assert!(
            shared
                .iter()
                .all(|operation| operation.id.as_str() != ListDirectories::ID)
        );
        assert_eq!(
            harmos::catalog::count(<schematic_guest::Plugin as Declared>::PROVIDES),
            3
        );
        assert_eq!(
            harmos::catalog::count(<schematic_sidecar::Plugin as Declared>::PROVIDES),
            4
        );
    });
}
