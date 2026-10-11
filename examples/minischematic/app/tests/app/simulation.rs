/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![cfg(feature = "sim")]
/* === Modules === */
#[path = "../../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeSet;
// Third Party Imports

// Local Imports
use harmos::PrincipalId;
use minischematic_app::app::{self, Simulated};
use minischematic_app::state::{NetId, NetName, PinRef, Point, Rotation, Schematic, SymbolId};
use minischematic_app::transactions::net::{ConnectPins, RenameNet};
use minischematic_app::transactions::symbol::{MoveSymbol, PlaceSymbol};

/* === Constants === */
const R1: SymbolId = SymbolId(1);
const U3: SymbolId = SymbolId(2);
const POWER: NetId = NetId(1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn scripted(seed: u64) -> Simulated {
    let mut editor = app::simulate(Schematic::default(), seed)
        .expect("the schematic declaration is simulatable");
    let alice = PrincipalId::new("alice");

    for (symbol, reference, x) in [(R1, "R1", 10), (U3, "U3", 60)] {
        editor
            .commit(
                alice.clone(),
                PlaceSymbol {
                    symbol,
                    reference: reference.to_owned(),
                    at: Point::new(x, 20),
                    rotation: Rotation::Zero,
                },
            )
            .expect("the simulated sheet starts empty");
    }
    editor
        .commit(
            alice.clone(),
            ConnectPins {
                net: POWER,
                name: NetName::verbatim("N$1"),
                pins: BTreeSet::from([PinRef::new(R1, 1), PinRef::new(U3, 7)]),
            },
        )
        .expect("the simulated symbols are placed");
    editor
        .commit(
            alice.clone(),
            MoveSymbol {
                symbol: R1,
                to: Point::new(10, 45),
            },
        )
        .expect("the placed symbol can move");
    editor
        .commit(
            alice,
            RenameNet {
                net: POWER,
                to: NetName::parse("VCC"),
            },
        )
        .expect("the connected net can be named");

    editor.stop().unwrap();
    editor.reboot().expect("the in-memory history reboots");
    editor
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("script_replays_restores", || {
            let first = scripted(42);
            let second = scripted(42);

            first.assert_same(&second);
            assert_eq!(first.trace(), second.trace());
            first.assert_replay_equivalent();
            first.assert_inverses_restore();
            assert_eq!(
                first
                    .state()
                    .expect("the rebooted schematic is readable")
                    .value
                    .net(POWER)
                    .name,
                NetName::parse("VCC")
            );
        });
    }
}
