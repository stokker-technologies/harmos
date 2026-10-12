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
use harmos::operations::{
    OperationDeclaration, OperationPolicy, RetryPolicy, Visibility, WireOperation,
};

/* === Constants === */
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(10),
    attempt_timeout: Duration::from_secs(10),
    retries: RetryPolicy::Never,
};

/* === Macros === */
macro_rules! operation {
    ($name:ident, $id:literal, $visibility:ident) => {
        pub struct $name;
        impl OperationDeclaration for $name {
            type Request = Blob;
            type Response = Blob;
            const ID: &'static str = $id;
            const POLICY: OperationPolicy = POLICY;
            const VISIBILITY: Visibility = Visibility::$visibility;
        }
        impl WireOperation for $name {
            const VERSION: u32 = 1;
        }
    };
}

operation!(Echo, "process.echo", Private);

operation!(Probe, "process.probe", Host);

operation!(Nested, "process.nested", Host);

/* === Type Aliases === */

/* === Structs === */
#[harmos::stream(id = "process.receipt", window = 8)]
#[harmos::message]
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Blob {
    pub id: u64,
    pub data: Vec<u8>,
}

/* === Enums === */
#[harmos::catalog]
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
