/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Share The Deterministic Script Between Stories And Benchmarks

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::support::{Bank, DEPOSIT, Deposit, NOTED, Noted, alice, pulse};
use harmos_runtime::{Builder, simulation::SimulatedRuntime};
use harmos_runtime::{DefinitionId, Identified};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Identified for Deposit {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new(DEPOSIT.1), DEPOSIT.2)
    }
}

impl Identified for Noted {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new(NOTED.1), NOTED.2)
    }
}

/* === Public Functions === */

pub(crate) fn scripted(builder: Builder<Bank>) -> SimulatedRuntime<Bank> {
    let mut sim = builder.simulated_service("adapter", resident).simulate().expect("the simulation boots");
    sim.commit(
        alice(),
        Deposit {
            account: super::support::HOUSE,
            amount: 7,
        },
    )
    .expect("the scripted commit lands");
    sim.record(alice(), Noted("seeded".to_owned())).expect("the scripted record lands");
    sim.publish(pulse(4)).expect("the row catalog is frozen");
    sim.fault("adapter", "bus reset").expect("the running service receives its exit");
    sim.advance_ms(12);
    sim
}

/* === Private Functions === */

async fn resident(mut context: harmos_runtime::services::ServiceContext<Bank>) -> Result<(), std::io::Error> {
    context.ready();
    context.stopping().await;
    Ok(())
}

/* === Tests === */
