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
use harmos_domain::StreamSpec;
use harmos_domain::listeners::Delivery;
use harmos_domain::operations::{
    OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, Visibility, WireOperation,
};

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};
pub const NOTE: &str = "example.note";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Heard;

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
pub struct Note {
    pub note: String,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
pub struct Progress {
    pub percent: u32,
}

#[harmos_guest::message(harmos_path = ::harmos_guest)]
#[derive(Clone, PartialEq)]
pub struct Metrics {
    pub count: i64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for Heard {
    type Request = Delivery<Note>;
    type Response = ();
    const ID: &'static str = "fixture.heard";
    const POLICY: OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
}

impl WireOperation for Heard {
    const VERSION: u32 = 1;
}

impl StreamSpec for Progress {
    type Row = Self;
    const ID: &'static str = "progress";
    const VERSION: u32 = 1;
    const WINDOW: usize = 16;
}

impl StreamSpec for Metrics {
    type Row = Self;
    const ID: &'static str = "metrics";
    const VERSION: u32 = 1;
    const WINDOW: usize = 16;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
