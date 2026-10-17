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
use harmos_runtime::operations::{OperationPolicy, RestartPolicy, RetryPolicy, Visibility};

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(10),
    attempt_timeout: Duration::from_secs(10),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */
macro_rules! operation {
    ($name:ident, $id:literal, $visibility:ident) => {
        #[harmos_runtime::message]
        #[harmos_runtime::operation(
                            id = $id,
                            version = 1,
                            policy = POLICY,
                            response = Blob,
                            visibility = Visibility::$visibility,
                        )]
        #[derive(Clone, PartialEq)]
        pub struct $name {
            pub blob: Blob,
        }
    };
}

operation!(Echo, "process.echo", Private);

operation!(Probe, "process.probe", Host);

operation!(Nested, "process.nested", Host);

/* === Type Aliases === */

/* === Structs === */
#[harmos_runtime::stream(id = "process.receipt", window = 8)]
#[harmos_runtime::message]
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Blob {
    pub id: u64,
    pub data: Vec<u8>,
}

/* === Enums === */
#[harmos_runtime::catalog]
pub enum Operations {
    Echo(Echo),
    Probe(Probe),
    Nested(Nested),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
