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
pub use provider::ProviderEchoOperation;
pub use read::ReadOperation;
pub use spin::SpinOperation;
pub use wait::WaitOperation;

/* === Imports === */

// Standard Library Imports
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos_domain::StreamSpec;
use harmos_domain::operations::{Backoff, Jitter, OperationPolicy, RetryPolicy};

/* === Constants === */

const PROBE: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
pub struct Echoed {
    pub said: String,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
pub struct Value {
    pub value: String,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
pub struct Stage {
    pub id: String,
}

/* === Enums === */

#[harmos_guest::catalog(harmos_path = ::harmos_guest)]
pub enum Operations {
    EchoOperation(EchoOperation),
    DeclineOperation(DeclineOperation),
    SpinOperation(SpinOperation),
    WaitOperation(WaitOperation),
    FailOperation(FailOperation),
    ReadOperation(ReadOperation),
    CountsOperation(CountsOperation),
}

/* === Traits === */

/* === Implementations === */

impl StreamSpec for Echoed {
    type Row = Self;
    const ID: &'static str = "progress";
    const VERSION: u32 = 1;
    const WINDOW: usize = 4;
}

impl harmos_guest::operations::ContextValue for Stage {
    const KEY: &'static str = "fixture.stage";
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
