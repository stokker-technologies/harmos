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
use std::time::Duration;
// Third Party Imports

// Local Imports

/* === Constants === */
pub const SDK_POLICY: harmos::OperationPolicy = harmos::OperationPolicy {
    timeout: Duration::from_secs(2), attempt_timeout: Duration::from_secs(1), retries: harmos::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Selected;

pub struct Unselected;

#[cfg(feature = "native")]
pub struct Start;

#[cfg(feature = "native")]
pub struct Stop;

#[harmos::message]
pub struct Request {
    value: u32,
}

#[harmos::message]
pub struct Response {
    value: u32,
}

/* === Enums === */
#[harmos::catalog]
pub enum Operations {
    Selected(Selected),
}

#[cfg(feature = "native")]
#[harmos::catalog]
pub enum NativeOperations {
    #[nested]
    Plugin(Operations),
    Start(Start),
    Stop(Stop),
}

/* === Traits === */

/* === Implementations === */
impl harmos::OperationDeclaration for Selected {
    type Request = Request;
    type Response = Response;
    const ID: &'static str = "selected.read";
    const DESCRIPTION: &'static str = "Selected \"reading\" μ";
    const VISIBILITY: harmos::operations::Visibility = harmos::operations::Visibility::Shared;
    const REPLAY: harmos::operations::ReplaySafety = harmos::operations::ReplaySafety::Idempotent;
    const POLICY: harmos::OperationPolicy = harmos::OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_nanos(17),
        retries: harmos::RetryPolicy::OnTransient {
            max_retries: 3,
            backoff: harmos::operations::Backoff::Fixed {
                delay: Duration::from_nanos(7),
                jitter: harmos::operations::Jitter::Deterministic,
            },
        },
    };
}

impl harmos::OperationDeclaration for Unselected {
    type Request = Request;
    type Response = Response;
    const ID: &'static str = "unselected.read";
    const POLICY: harmos::OperationPolicy = <Selected as harmos::OperationDeclaration>::POLICY;
}

impl harmos::operations::WireOperation for Selected { const VERSION: u32 = 3; }

impl harmos::operations::WireOperation for Unselected { const VERSION: u32 = 1; }

#[cfg(feature = "native")]
impl harmos::OperationDeclaration for Start {
    type Request = harmos::operations::observers::Control;
    type Response = ();
    const ID: &'static str = "fixture.start";
    const POLICY: harmos::OperationPolicy = SDK_POLICY;
    const VISIBILITY: harmos::operations::Visibility = harmos::operations::Visibility::Host;
}

#[cfg(feature = "native")]
impl harmos::operations::WireOperation for Start { const VERSION: u32 = 1; }

#[cfg(feature = "native")]
impl harmos::OperationDeclaration for Stop {
    type Request = harmos::operations::observers::Control;
    type Response = harmos::operations::observers::Status;
    const ID: &'static str = "fixture.stop";
    const POLICY: harmos::OperationPolicy = SDK_POLICY;
    const VISIBILITY: harmos::operations::Visibility = harmos::operations::Visibility::Host;
}

#[cfg(feature = "native")]
impl harmos::operations::WireOperation for Stop { const VERSION: u32 = 1; }

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
