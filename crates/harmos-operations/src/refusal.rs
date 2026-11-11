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
use std::fmt;
// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use crate::failure::{Failure, Origin};
use crate::operations;

/* === Constants === */

pub const RESERVED: &str = "harmos.";

pub const FAILED: i64 = -32_000;

pub const UNKNOWN: i64 = -32_601;

pub const UNREADABLE: i64 = -32_602;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// A host or carrier refusal: the JSON-RPC code beside the failure it carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refusal {
    code: i64,
    failure: Failure,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Refusal {
    pub fn new(code: i64, message: impl fmt::Display) -> Self {
        let failure = Failure::new(operations::FailureKind::Permanent, message.to_string());
        Self::failed(code, failure)
    }

    pub fn of(cause: impl fmt::Display) -> Self {
        Self::new(FAILED, cause)
    }

    pub fn failed(code: i64, failure: Failure) -> Self {
        Self {
            code,
            failure: failure.with_refusal_code(code),
        }
    }

    // Refuses with the failure an error chain carries, or writes its cause.
    pub fn originate(
        code: i64, classification: operations::FailureKind, error: &(dyn std::error::Error + 'static), origin: Origin,
    ) -> Self {
        Self::failed(code, Failure::originate(classification, error, origin))
    }

    pub fn code(&self) -> i64 {
        self.code
    }

    pub fn message(&self) -> &str {
        self.failure.cause()
    }

    pub fn failure(&self) -> &Failure {
        &self.failure
    }

    pub fn into_failure(self) -> Failure {
        self.failure
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A Refusal Is A Leaf: It Names The Operation Frame Beside Its Cause
        match self.failure.context().operation() {
            Some(operation) => write!(formatter, "operation `{operation}` failed: {} ({})", self.failure.cause(), self.code),
            None => write!(formatter, "{} ({})", self.failure.cause(), self.code),
        }
    }
}

impl std::error::Error for Refusal {}

/* === Public Functions === */

pub const fn reserved(method: &str) -> bool {
    let bytes = method.as_bytes();
    bytes.len() >= 7
        && bytes[0] == b'h'
        && bytes[1] == b'a'
        && bytes[2] == b'r'
        && bytes[3] == b'm'
        && bytes[4] == b'o'
        && bytes[5] == b's'
        && (bytes[6] == b'.' || bytes[6] == b'_')
}

/* === Private Functions === */

/* === Tests === */
