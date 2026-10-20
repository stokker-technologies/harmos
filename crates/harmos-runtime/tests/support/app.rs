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
use harmos_runtime::{Builder, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub async fn runtime() -> Runtime<Bank> {
    booted(Ledger::default(), |builder| builder).await
}

pub async fn streaming() -> Runtime<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(streams())
        .runtime()
        .await
        .expect("a memory-only boot recovers nothing")
}

pub async fn runtime_from(origin: Ledger) -> Runtime<Bank> {
    booted(origin, |builder| builder).await
}

pub async fn booted(origin: Ledger, attach: impl FnOnce(Builder<Bank>) -> Builder<Bank>) -> Runtime<Bank> {
    attach(Runtime::<Bank>::builder(origin, ()).register(catalog())).runtime().await.expect("a memory-only boot recovers nothing")
}

/* === Private Functions === */

/* === Tests === */
