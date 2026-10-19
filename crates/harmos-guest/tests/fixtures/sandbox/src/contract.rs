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
use harmos_domain::operations::{Backoff, Jitter, OperationPolicy, RestartPolicy, RetryPolicy, Visibility};

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
#[harmos_domain::operation(id = "probe.echo", version = 1, response = Echoed, policy = PROBE, visibility = Visibility::Shared)]
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
#[harmos_domain::operation(id = "probe.decline", version = 1, response = Echoed, policy = PROBE)]
pub struct Decline {}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
#[harmos_domain::operation(id = "probe.spin", version = 1, response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_secs(600),
        attempt_timeout: Duration::from_secs(600),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    })]
pub struct Spin {}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
#[harmos_domain::operation(id = "probe.wait", version = 1, response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_millis(5),
        attempt_timeout: Duration::from_millis(5),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    })]
pub struct Wait {}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
#[harmos_domain::operation(id = "probe.fail", version = 1, response = Echoed, policy = PROBE)]
pub struct Fail {}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
pub struct Value {
    pub value: String,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
pub struct Stage {
    pub id: String,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
#[harmos_domain::operation(id = "provider.echo", version = 1, response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_millis(100),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    }, visibility = Visibility::Shared)]
pub struct ProviderEcho {
    pub say: String,
}
#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
#[harmos_domain::operation(id = "fixture.read", version = 1, response = Value, policy = OperationPolicy {
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
    })]
pub struct Read {
    pub value: String,
}
#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
#[harmos_domain::operation(id = "fixture.counts", version = 1, response = Value, policy = Read::POLICY)]
pub struct Counts {
    pub value: String,
}

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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
