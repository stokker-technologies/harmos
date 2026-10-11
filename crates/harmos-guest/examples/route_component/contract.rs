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
use harmos_contract::StreamSpec;
use harmos_contract::operations::{
    OperationDeclaration, OperationPolicy, RetryPolicy, WireOperation,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct ProviderEcho;

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
pub struct Echo {
    pub say: String,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
pub struct Echoed {
    pub said: String,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
pub struct Decline {}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
pub struct Spin {}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
pub struct Wait {}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
pub struct Fail {}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl StreamSpec for Echoed {
    type Row = Self;
    const ID: &'static str = "progress";
    const VERSION: u32 = 1;
    const WINDOW: usize = 4;
}

impl OperationDeclaration for ProviderEcho {
    const VISIBILITY: harmos_guest::operations::Visibility =
        harmos_guest::operations::Visibility::Shared;
    type Request = Echo;
    type Response = Echoed;
    const ID: &'static str = "provider.echo";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_millis(100),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
    };
}

impl OperationDeclaration for Echo {
    const VISIBILITY: harmos_guest::operations::Visibility =
        harmos_guest::operations::Visibility::Shared;
    type Request = Self;
    const ID: &'static str = "probe.echo";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    };

    type Response = Echoed;
}

impl OperationDeclaration for Decline {
    type Request = Self;
    const ID: &'static str = "probe.decline";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    };

    type Response = Echoed;
}

impl OperationDeclaration for Spin {
    type Request = Self;
    const ID: &'static str = "probe.spin";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(600),
        attempt_timeout: Duration::from_secs(600),
        retries: RetryPolicy::Never,
    };

    type Response = Echoed;
}

impl OperationDeclaration for Wait {
    type Request = Self;
    const ID: &'static str = "probe.wait";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_millis(5),
        attempt_timeout: Duration::from_millis(5),
        retries: RetryPolicy::Never,
    };

    type Response = Echoed;
}

impl OperationDeclaration for Fail {
    type Request = Self;
    const ID: &'static str = "probe.fail";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    };

    type Response = Echoed;
}

impl WireOperation for ProviderEcho {
    const VERSION: u32 = 1;
}

impl WireOperation for Echo {
    const VERSION: u32 = 1;
}

impl WireOperation for Decline {
    const VERSION: u32 = 1;
}

impl WireOperation for Spin {
    const VERSION: u32 = 1;
}

impl WireOperation for Wait {
    const VERSION: u32 = 1;
}

impl WireOperation for Fail {
    const VERSION: u32 = 1;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
