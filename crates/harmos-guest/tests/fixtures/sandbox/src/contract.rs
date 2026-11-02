/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Sandbox Guest's Declarations, Included By The Host Stories That Drive It
#![allow(dead_code)]

/* === Modules === */

#[path = "contract/counts.rs"]
mod counts;
#[path = "contract/decline.rs"]
mod decline;
#[path = "contract/echo.rs"]
mod echo;
#[path = "contract/fail.rs"]
mod fail;
#[path = "contract/finish.rs"]
mod finish;
#[path = "contract/provider.rs"]
mod provider;
#[path = "contract/read.rs"]
mod read;
#[path = "contract/spin.rs"]
mod spin;
#[path = "contract/wait.rs"]
mod wait;

/* === Re-Exports === */

pub use counts::CountsOperation;
pub use decline::DeclineOperation;
pub use echo::EchoOperation;
pub use fail::FailOperation;
pub use finish::FinishOperation;
pub use provider::ProviderEchoOperation;
pub use read::ReadOperation;
pub use spin::SpinOperation;
pub use wait::WaitOperation;

/* === Imports === */

// Standard Library Imports
use std::time::Duration;
// Third Party Imports

// Local Imports
#[cfg(target_arch = "wasm32")]
use harmos::domain as harmos_domain;
#[cfg(target_arch = "wasm32")]
use harmos::guest;
use harmos_domain::StreamSpec;
use harmos_domain::operations::{Backoff, Jitter, OperationPolicy, RetryPolicy};
#[cfg(not(target_arch = "wasm32"))]
use harmos_guest as guest;

/* === Constants === */

const PROBE: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[guest::message(harmos_path = guest)]
#[derive(Clone, PartialEq)]
pub struct Echoed {
    pub said: String,
}

#[guest::message(harmos_path = guest)]
pub struct Value {
    pub value: String,
}

#[guest::message(harmos_path = guest)]
pub struct Stage {
    pub id: String,
}

/* === Enums === */

#[cfg_attr(not(target_arch = "wasm32"), harmos_domain::catalog(harmos_path = ::harmos_domain))]
#[cfg_attr(target_arch = "wasm32", harmos::guest::catalog)]
pub enum Operations {
    EchoOperation(EchoOperation),
    DeclineOperation(DeclineOperation),
    SpinOperation(SpinOperation),
    WaitOperation(WaitOperation),
    FailOperation(FailOperation),
    ReadOperation(ReadOperation),
    CountsOperation(CountsOperation),
    FinishOperation(FinishOperation),
}

/* === Traits === */

/* === Implementations === */

impl StreamSpec for Echoed {
    type Row = Self;
    const ID: &'static str = "progress";
    const VERSION: u32 = 1;
    const WINDOW: usize = 4;
}

impl guest::operations::ContextValue for Stage {
    const KEY: &'static str = "fixture.stage";
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
