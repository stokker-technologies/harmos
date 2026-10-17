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

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos_domain::StreamSpec;
use harmos_domain::operations::{
    Backoff, Jitter, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Visibility,
    WireOperation,
};

/* === Constants === */
const PROBE: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
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

#[harmos_guest::message(harmos_path = ::harmos_guest)]
pub struct Value {
    pub value: String,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
pub struct Stage {
    pub id: String,
}

pub struct ProviderEcho;
pub struct Read;
pub struct Counts;

/* === Enums === */
#[harmos_guest::catalog(harmos_path = ::harmos_guest)]
pub enum Operations {
    Echo(Echo),
    Decline(Decline),
    Spin(Spin),
    Wait(Wait),
    Fail(Fail),
    Read(Read),
    Counts(Counts),
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

impl OperationDeclaration for ProviderEcho {
    const VISIBILITY: Visibility = Visibility::Shared;
    type Request = Echo;
    type Response = Echoed;
    const ID: &'static str = "provider.echo";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_millis(100),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
}

impl OperationDeclaration for Echo {
    const VISIBILITY: Visibility = Visibility::Shared;
    type Request = Self;
    type Response = Echoed;
    const ID: &'static str = "probe.echo";
    const POLICY: OperationPolicy = PROBE;
}

impl OperationDeclaration for Decline {
    type Request = Self;
    type Response = Echoed;
    const ID: &'static str = "probe.decline";
    const POLICY: OperationPolicy = PROBE;
}

impl OperationDeclaration for Spin {
    type Request = Self;
    type Response = Echoed;
    const ID: &'static str = "probe.spin";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(600),
        attempt_timeout: Duration::from_secs(600),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
}

impl OperationDeclaration for Wait {
    type Request = Self;
    type Response = Echoed;
    const ID: &'static str = "probe.wait";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_millis(5),
        attempt_timeout: Duration::from_millis(5),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
}

impl OperationDeclaration for Fail {
    type Request = Self;
    type Response = Echoed;
    const ID: &'static str = "probe.fail";
    const POLICY: OperationPolicy = PROBE;
}

impl OperationDeclaration for Read {
    type Request = Value;
    type Response = Value;
    const ID: &'static str = "fixture.read";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_millis(500),
        retries: RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: Jitter::None,
            },
        },
        restart: RestartPolicy::Never,
    };
}

impl OperationDeclaration for Counts {
    type Request = Value;
    type Response = Value;
    const ID: &'static str = "fixture.counts";
    const POLICY: OperationPolicy = Read::POLICY;
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

impl WireOperation for Read {
    const VERSION: u32 = 1;
}

impl WireOperation for Counts {
    const VERSION: u32 = 1;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
