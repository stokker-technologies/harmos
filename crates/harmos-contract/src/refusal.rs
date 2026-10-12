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

// Local Imports

/* === Constants === */
pub const RESERVED: &str = "harmos.";

const FAILED: i64 = -32_000;

pub const UNKNOWN: i64 = -32_601;

pub const UNREADABLE: i64 = -32_602;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    code: i64,
    message: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Refusal {
    pub fn new(code: i64, message: impl fmt::Display) -> Self {
        Self {
            code,
            message: message.to_string(),
        }
    }

    pub fn of(cause: impl fmt::Display) -> Self {
        Self::new(FAILED, cause)
    }

    pub fn code(&self) -> i64 {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for Refusal {}

/* === Public Functions === */
pub const fn reserved(method: &str) -> bool {
    let method = method.as_bytes();
    let prefix = RESERVED.as_bytes();
    if method.len() < prefix.len() {
        return false;
    }

    let mut index = 0;
    while index < prefix.len() {
        if method[index] != prefix[index] {
            return false;
        }
        index += 1;
    }

    true
}

/* === Private Functions === */

/* === Tests === */
