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

#[harmos_guest::operation(
    harmos_path = ::harmos_guest,
    id = "probe_echo",
    response = Echoed,
    policy = PROBE,
    visibility = Shared,
)]
#[derive(Clone, PartialEq)]

pub struct EchoOperation {
    pub say: String,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
pub struct Echoed {
    pub said: String,
}

#[harmos_guest::operation(harmos_path = ::harmos_guest, id = "probe_decline", response = Echoed, policy = PROBE)]
#[derive(Clone, PartialEq)]

pub struct DeclineOperation {}

#[harmos_guest::operation(harmos_path = ::harmos_guest, id = "probe_spin", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_secs(600),
        attempt_timeout: Duration::from_secs(600),
        retries: RetryPolicy::Never,
    })]
#[derive(Clone, PartialEq)]

pub struct SpinOperation {}

#[harmos_guest::operation(harmos_path = ::harmos_guest, id = "probe_wait", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_millis(5),
        attempt_timeout: Duration::from_millis(5),
        retries: RetryPolicy::Never,
    })]
#[derive(Clone, PartialEq)]

pub struct WaitOperation {}

#[harmos_guest::operation(harmos_path = ::harmos_guest, id = "probe_fail", response = Echoed, policy = PROBE)]
#[derive(Clone, PartialEq)]

pub struct FailOperation {}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
pub struct Value {
    pub value: String,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
pub struct Stage {
    pub id: String,
}

#[harmos_guest::operation(harmos_path = ::harmos_guest, id = "provider_echo", response = Echoed, policy = OperationPolicy {
        timeout: Duration::from_millis(100),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
    }, visibility = Shared)]
#[derive(Clone, PartialEq)]

pub struct ProviderEchoOperation {
    pub say: String,
}
#[harmos_guest::operation(harmos_path = ::harmos_guest, id = "fixture_read", response = Value, policy = OperationPolicy {
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
#[derive(Clone, PartialEq)]

pub struct ReadOperation {
    pub value: String,
}
#[harmos_guest::operation(harmos_path = ::harmos_guest, id = "fixture_counts", response = Value, policy = ReadOperation::POLICY)]
#[derive(Clone, PartialEq)]

pub struct CountsOperation {
    pub value: String,
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
