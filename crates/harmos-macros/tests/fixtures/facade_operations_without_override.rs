/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod relative {
    #[facade::message]
    pub struct WireReply {
        pub value: String,
    }
    pub type Reply = WireReply;
}
mod rows {
    pub use facade::Reading;
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::Duration;
// Third Party Imports
use facade::__harmos::operations::{OperationDeclaration, OperationPolicy, RetryPolicy};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct State;

#[facade::operation(
    response = relative::Reply,
    id = "test.echo",
    policy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::Never,
},
)]
pub struct Echo {
    pub value: String,
}

pub struct Guest;

/* === Enums === */

#[facade::catalog]
pub enum Operations {
    Echo(Echo),
}

/* === Traits === */

trait ProductPlugin {}

/* === Implementations === */

impl facade::__harmos::builtins::lifecycle::Policy for State {
    const INITIALIZATION_POLICY: OperationPolicy = Echo::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Echo::POLICY;
}

impl facade::__harmos::builtins::lifecycle::Policy for Guest {
    const INITIALIZATION_POLICY: OperationPolicy = Echo::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Echo::POLICY;
}

#[facade::plugin(id = "alias", context = facade::__harmos::guest::kit::Context, config = String,
    streams(self::rows::Reading), provides = Operations, requires = ())]
impl ProductPlugin for Guest {}

#[facade::plugin(id = "alias", context = facade::__harmos::sidecar::Supervisor, config = String,
    restart = "never", streams(self::rows::Reading), provides = Operations, requires = ())]
impl ProductPlugin for State {}

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
