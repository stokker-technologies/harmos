/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


mod relative {
    #[facade::message(harmos_path = ::facade::__harmos)]
    pub struct WireReply {
        pub value: String,
    }
    pub type Reply = WireReply;
}


use std::time::Duration;

use facade::__harmos::operations::{OperationPolicy, RetryPolicy};


#[facade::operation(
    harmos_path = ::facade::__harmos,
    response = relative::Reply,
    id = "test.echo",
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::Never,
    },
)]
pub struct Echo {
    pub value: String,
}


#[facade::catalog(harmos_path = ::facade::__harmos)]
pub enum Operations {
    Echo(Echo),
}


fn main() {}
