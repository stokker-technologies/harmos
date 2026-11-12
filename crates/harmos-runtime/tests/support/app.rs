/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{Bank, Ledger, catalog, streams};
use harmos_runtime::{Runtime, Setup};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub async fn runtime() -> Runtime<Bank> {
    booted(Ledger::default(), |_| {}).await
}

pub async fn streaming() -> Runtime<Bank> {
    booted(Ledger::default(), |setup| {
        setup.settings().register(streams());
    })
    .await
}

pub async fn runtime_from(origin: Ledger) -> Runtime<Bank> {
    booted(origin, |_| {}).await
}

pub async fn booted(origin: Ledger, attach: impl FnOnce(&mut Setup<Bank>)) -> Runtime<Bank> {
    launch::<Bank>(origin, (), |setup| {
        setup.settings().register(catalog());
        attach(setup);
    })
    .await
    .expect("a memory-only boot recovers nothing")
}

pub async fn launch<A: harmos_runtime::Application>(
    origin: impl Into<harmos_runtime::state::Origin<A::State>>, config: A::Config, attach: impl FnOnce(&mut Setup<A>),
) -> Result<Runtime<A>, harmos_runtime::runtime::StartupError> {
    Runtime::<A>::launch_with(origin, config, attach).await
}

pub async fn register<A: harmos_runtime::Application>(
    runtime: &Runtime<A>, catalog: harmos_runtime::operations::Catalog, lane: harmos_runtime::operations::Lane,
) -> harmos_runtime::Result<()> {
    runtime.operations.register_catalog(catalog, lane).await
}

pub async fn shutdown<A: harmos_runtime::Application>(runtime: &Runtime<A>) -> harmos_runtime::Result<harmos_runtime::state::Position> {
    runtime.shutdown().await
}

/* === Private Functions === */

/* === Tests === */
