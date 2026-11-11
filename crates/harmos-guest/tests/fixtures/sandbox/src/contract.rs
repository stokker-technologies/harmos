/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Sandbox Guest's Declarations, Included by the Host Stories That Drive It
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

#[cfg(target_arch = "wasm32")]
pub use counts::counts_operation_operation;
pub use counts::{COUNTS_OPERATION_OPERATION, CountsOperation};
pub use decline::DeclineOperation;
#[cfg(target_arch = "wasm32")]
pub use decline::decline_operation_operation;
pub const DECLINE_OPERATION_OPERATION: guest::operations::OperationSpec<DeclineOperation> = decline::DECLINE_OPERATION_OPERATION;
#[cfg(target_arch = "wasm32")]
pub use echo::echo_operation_operation;
pub use echo::{ECHO_OPERATION_OPERATION, EchoOperation};
pub use fail::FailOperation;
#[cfg(target_arch = "wasm32")]
pub use fail::fail_operation_operation;
pub const FAIL_OPERATION_OPERATION: guest::operations::OperationSpec<FailOperation> = fail::FAIL_OPERATION_OPERATION;
pub use finish::FinishOperation;
#[cfg(target_arch = "wasm32")]
pub use finish::finish_operation_operation;
pub const FINISH_OPERATION_OPERATION: guest::operations::OperationSpec<FinishOperation> = finish::FINISH_OPERATION_OPERATION;
pub use provider::ProviderEchoOperation;
pub const PROVIDER_ECHO_OPERATION_OPERATION: guest::operations::OperationSpec<ProviderEchoOperation> =
    provider::PROVIDER_ECHO_OPERATION_OPERATION;
#[cfg(target_arch = "wasm32")]
pub use read::read_operation_operation;
pub use read::{READ_OPERATION_OPERATION, ReadOperation};
pub use spin::SpinOperation;
#[cfg(target_arch = "wasm32")]
pub use spin::spin_operation_operation;
pub const SPIN_OPERATION_OPERATION: guest::operations::OperationSpec<SpinOperation> = spin::SPIN_OPERATION_OPERATION;
pub use wait::WaitOperation;
#[cfg(target_arch = "wasm32")]
pub use wait::wait_operation_operation;
pub const WAIT_OPERATION_OPERATION: guest::operations::OperationSpec<WaitOperation> = wait::WAIT_OPERATION_OPERATION;

/* === Imports === */

// Standard Library Imports
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos_guest as guest;
#[cfg(target_arch = "wasm32")]
use harmos_guest as harmos_domain;
use harmos_operations::StreamSpec;
use harmos_operations::operations::{Backoff, Jitter, OperationPolicy, RetryPolicy};

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

#[derive(guest::Message, Debug, Default)]
#[harmos_path(guest)]
#[derive(Clone, PartialEq)]
pub struct Echoed {
    pub said: String,
}

#[derive(guest::Message, Clone, Debug, Default)]
#[harmos_path(guest)]
pub struct Value {
    pub value: String,
}

#[derive(guest::Message, Clone, Debug, Default)]
#[harmos_path(guest)]
pub struct Stage {
    pub id: String,
}

#[derive(guest::Message, Clone, Debug, Default)]
#[harmos_path(guest)]
pub struct RetryChoice {
    pub retries: u32,
    pub delay_millis: u64,
}
impl guest::operations::ContextValue for RetryChoice {
    const KEY: &'static str = "fixture.retry_choice";
}
impl RetryChoice {
    pub fn policy(&self) -> RetryPolicy {
        RetryPolicy::OnTransient {
            max_retries: usize::try_from(self.retries).expect("retry count fits this target"),
            backoff: Backoff::Fixed {
                delay: Duration::from_millis(self.delay_millis),
                jitter: Jitter::None,
            },
        }
    }
}

/* === Enums === */

static OPERATIONS_CATALOG_DECLARATIONS: [harmos_guest::catalog::Node<harmos_guest::operations::WireDeclaration>; 8] = [
    harmos_guest::catalog::Node::Leaf(ECHO_OPERATION_OPERATION.wire()),
    harmos_guest::catalog::Node::Leaf(DECLINE_OPERATION_OPERATION.wire()),
    harmos_guest::catalog::Node::Leaf(SPIN_OPERATION_OPERATION.wire()),
    harmos_guest::catalog::Node::Leaf(WAIT_OPERATION_OPERATION.wire()),
    harmos_guest::catalog::Node::Leaf(FAIL_OPERATION_OPERATION.wire()),
    harmos_guest::catalog::Node::Leaf(READ_OPERATION_OPERATION.wire()),
    harmos_guest::catalog::Node::Leaf(COUNTS_OPERATION_OPERATION.wire()),
    harmos_guest::catalog::Node::Leaf(FINISH_OPERATION_OPERATION.wire()),
];
static OPERATIONS_CATALOG_CALLEES: [harmos_guest::catalog::Node<&'static [harmos_guest::operations::WireDeclaration]>; 8] = [
    harmos_guest::catalog::Node::Leaf(ECHO_OPERATION_OPERATION.callees()),
    harmos_guest::catalog::Node::Leaf(DECLINE_OPERATION_OPERATION.callees()),
    harmos_guest::catalog::Node::Leaf(SPIN_OPERATION_OPERATION.callees()),
    harmos_guest::catalog::Node::Leaf(WAIT_OPERATION_OPERATION.callees()),
    harmos_guest::catalog::Node::Leaf(FAIL_OPERATION_OPERATION.callees()),
    harmos_guest::catalog::Node::Leaf(READ_OPERATION_OPERATION.callees()),
    harmos_guest::catalog::Node::Leaf(COUNTS_OPERATION_OPERATION.callees()),
    harmos_guest::catalog::Node::Leaf(FINISH_OPERATION_OPERATION.callees()),
];

pub const OPERATIONS_CATALOG: harmos_guest::operations::CatalogDescription = harmos_guest::operations::CatalogDescription::new(
    concat!(module_path!(), "::Operations"),
    &OPERATIONS_CATALOG_DECLARATIONS,
    &OPERATIONS_CATALOG_CALLEES,
    OPERATIONS_CATALOG_FACTORY,
);

/* === Traits === */

/* === Implementations === */

impl StreamSpec for Echoed {
    type Row = Self;
    const ENTRY: harmos_domain::StreamEntry<Self> = harmos_domain::StreamEntry::new("progress", 4);
}

impl guest::operations::ContextValue for Stage {
    const KEY: &'static str = "fixture.stage";
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(not(target_arch = "wasm32"))]
use declared_catalogs::OPERATIONS_CATALOG_FACTORY;
#[cfg(target_arch = "wasm32")]
use registered_catalogs::OPERATIONS_CATALOG_FACTORY;

#[cfg(target_arch = "wasm32")]
mod registered_catalogs {
    use super::*;
    pub(super) const OPERATIONS_CATALOG_FACTORY: Option<fn() -> harmos_guest::__private::Catalog> = Some(operations_catalog_batch);
    fn operations_catalog_batch() -> harmos_guest::__private::Catalog {
        let mut batch = harmos_guest::__private::Catalog::new();
        batch = batch.entry(echo_operation_operation().expect("representable operation"));
        batch = batch.entry(decline_operation_operation().expect("representable operation"));
        batch = batch.entry(spin_operation_operation().expect("representable operation"));
        batch = batch.entry(wait_operation_operation().expect("representable operation"));
        batch = batch.entry(fail_operation_operation().expect("representable operation"));
        batch = batch.entry(read_operation_operation().expect("representable operation"));
        batch = batch.entry(counts_operation_operation().expect("representable operation"));
        batch = batch.entry(finish_operation_operation().expect("representable operation"));
        batch
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod declared_catalogs {
    pub(super) const OPERATIONS_CATALOG_FACTORY: Option<fn() -> harmos_guest::__private::Catalog> = None;
}
