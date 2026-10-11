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
    #[facade::message(harmos_path = ::facade::__harmos)]
    pub struct WireReply { pub value: String }
    pub type Reply = WireReply;
}
mod rows { pub use facade::Reading; }

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
pub type Request = WireRequest;

/* === Structs === */
#[facade::message(harmos_path = ::facade::__harmos)]
pub struct WireRequest { pub value: String }

pub struct State;

pub struct Echo;

pub struct Guest;

/* === Enums === */
#[facade::catalog(harmos_path = ::facade::__harmos)]
pub enum Operations { Echo(Echo) }

/* === Traits === */
trait ProductPlugin {}

/* === Implementations === */
impl OperationDeclaration for Echo {
    type Request = Request;
    type Response = relative::Reply;
    const ID: &'static str = "alias.echo";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::Never,
    };
}

impl facade::__harmos::operations::WireOperation for Echo { const VERSION: u32 = 1; }

impl facade::__harmos::operations::lifecycle::Policy for State {
    const INITIALIZATION_POLICY: OperationPolicy = Echo::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Echo::POLICY;
}

impl facade::__harmos::operations::lifecycle::Policy for Guest {
    const INITIALIZATION_POLICY: OperationPolicy = Echo::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Echo::POLICY;
}

#[facade::plugin(id = "alias", context = facade::__harmos::guest::Context, config = String,
    streams(self::rows::Reading), provides = Operations, requires = (), harmos_path = ::facade::__harmos)]
impl ProductPlugin for Guest {}

#[facade::plugin(id = "alias", context = facade::__harmos::sidecar::Supervisor, config = String,
    restart = "never", streams(self::rows::Reading), provides = Operations, requires = (), harmos_path = ::facade::__harmos)]
impl ProductPlugin for State {}

/* === Public Functions === */

/* === Private Functions === */
fn main() {}

/* === Tests === */
