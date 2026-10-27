/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Inspect Guest Component World And Inventory

/* === Modules === */

#[path = "../../../harmos-runtime/tests/support/fixtures.rs"]
mod fixtures;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_domain::inventory::{Declaration, Facility};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn world(component: &[u8]) -> String {
    let decoded = wit_component::decode(component).expect("the fixture is a real component");
    let wit_component::DecodedWasm::Component(resolve, world) = decoded else {
        panic!("a component decodes as a component");
    };
    let package = resolve.worlds[world].package.expect("a decoded world belongs to a package");
    let mut printer = wit_component::WitPrinter::default();
    printer.print(&resolve, package, &[]).expect("the decoded world prints");

    printer.output.to_string()
}

fn digest(world: &str) -> u64 {
    world.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3))
}

/* === Tests === */

#[test]
fn world_hash_unchanged() {
    // Different Operation Catalogs Do Not Change The Component World
    let schematic = world(&fixtures::inspection("schematic-guest"));
    let sandbox = world(&fixtures::inspection("harmos-sandbox-guest"));

    assert_eq!(digest(&schematic), digest(&sandbox), "declaring other operations moved the world:\n{schematic}\n---\n{sandbox}");
    assert!(sandbox.contains("operate"), "the world both guests carry is the one with `operate` on it:\n{sandbox}");
}

#[test]
fn inventory_owner_isolation() {
    let bytes = fixtures::inspection("schematic-guest");
    let declared = Declaration::inspect(&bytes, Facility::Guest).expect("the real Wasm inventory reads without instantiation");
    assert_eq!(declared.id, "schematic_tools");
    assert!(declared.contracts.requires.is_empty());
    assert_eq!(
        declared.operations.iter().map(|operation| (operation.id.as_str(), operation.policy.timeout.as_millis())).collect::<Vec<_>>(),
        [("schematic.validate", 10000), ("schematic.route", 10000), ("schematic.analyze", 10000),]
    );
}
