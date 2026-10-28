/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::{Bank, Ledger, catalog, streams};
use harmos_runtime::{Runtime, Setup};

pub async fn runtime() -> Runtime<Bank> {
    booted(Ledger::default(), |_| {}).await
}

pub async fn streaming() -> Runtime<Bank> {
    Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().register(catalog());
        setup.settings().register(streams());
    })
    .await
    .expect("a memory-only boot recovers nothing")
}

pub async fn runtime_from(origin: Ledger) -> Runtime<Bank> {
    booted(origin, |_| {}).await
}

pub async fn booted(origin: Ledger, attach: impl FnOnce(&mut Setup<Bank>)) -> Runtime<Bank> {
    Runtime::<Bank>::launch_with(origin, (), |setup| {
        setup.settings().register(catalog());
        attach(setup);
    })
    .await
    .expect("a memory-only boot recovers nothing")
}
