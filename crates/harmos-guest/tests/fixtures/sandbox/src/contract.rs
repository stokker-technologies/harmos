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
use harmos_domain::operations::{Backoff, Jitter, OperationPolicy, RetryPolicy, Visibility};

/* === Constants === */

const PROBE: OperationPolicy =
    OperationPolicy { timeout: Duration::from_secs(2), attempt_timeout: Duration::from_secs(2), retries: RetryPolicy::Never };

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]

pub struct Echo {
    pub say: String,
}

#[harmos_domain::operation(request = Echo, infer_request, id = "probe_echo", response = Echoed, policy = PROBE, visibility = Visibility::Shared)]
pub struct EchoOperation;

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
pub struct Echoed {
    pub said: String,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]

pub struct Decline {}

#[harmos_domain::operation(request = Decline, infer_request, id = "probe_decline", response = Echoed, policy = PROBE)]
pub struct DeclineOperation;

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]

pub struct Spin {}

#[harmos_domain::operation(request = Spin, infer_request, id = "probe_spin", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_secs(600),
        attempt_timeout: Duration::from_secs(600),
        retries: RetryPolicy::Never,

    })]
pub struct SpinOperation;

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]

pub struct Wait {}

#[harmos_domain::operation(request = Wait, infer_request, id = "probe_wait", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_millis(5),
        attempt_timeout: Duration::from_millis(5),
        retries: RetryPolicy::Never,

    })]
pub struct WaitOperation;

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]

pub struct Fail {}

#[harmos_domain::operation(request = Fail, infer_request, id = "probe_fail", response = Echoed, policy = PROBE)]
pub struct FailOperation;

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

pub struct ProviderEcho {
    pub say: String,
}

#[harmos_domain::operation(request = ProviderEcho, infer_request, id = "provider_echo", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_millis(100),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,

    }, visibility = Visibility::Shared)]
pub struct ProviderEchoOperation;
#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]

pub struct Read {
    pub value: String,
}

#[harmos_domain::operation(request = Read, infer_request, id = "fixture_read", response = Value, policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_millis(500),
        retries: RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: Jitter::None,
            },
        },

    })]
pub struct ReadOperation;
#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]

pub struct Counts {
    pub value: String,
}

#[harmos_domain::operation(request = Counts, infer_request, id = "fixture_counts", response = Value, policy = ReadOperation::POLICY)]
pub struct CountsOperation;

/* === Enums === */

#[harmos_guest::catalog(harmos_path = ::harmos_guest)]
pub enum Operations {
    Echo(EchoOperation),
    Decline(DeclineOperation),
    Spin(SpinOperation),
    Wait(WaitOperation),
    Fail(FailOperation),
    Read(ReadOperation),
    Counts(CountsOperation),
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
