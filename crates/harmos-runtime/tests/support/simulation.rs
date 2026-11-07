/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Share the Deterministic Script Between Stories and Benchmarks

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::support::{Bank, Deposit, Noted, alice, pulse};
use harmos_runtime::Identified;
use harmos_runtime::{Setup, simulation::SimulatedRuntime};

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
    const ID: &'static str = "ledger.deposit";
}

impl Identified for Noted {
    const ID: &'static str = "ledger.noted";
}

/* === Public Functions === */

pub(crate) fn configure(setup: &mut Setup<Bank>) {
    setup.settings().simulated_service("adapter", resident);
}

pub(crate) fn scripted(mut sim: SimulatedRuntime<Bank>) -> SimulatedRuntime<Bank> {
    sim.commit(
        alice(),
        Deposit {
            account: super::support::HOUSE,
            amount: 7,
        },
    )
    .expect("the scripted commit lands");
    let record = Noted("seeded".to_owned());
    sim.record(alice(), record).expect("the scripted record lands");
    let row = pulse(4);
    sim.publish(row).expect("the row catalog is frozen");
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
