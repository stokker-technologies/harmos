/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[facade::message(harmos_path = ::facade::__harmos)]
#[derive(facade::Request, Clone)]
#[harmos_path(::facade::__harmos)]
#[request(response = relative::Reply)]
pub struct Echo { pub value: String }
mod relative {
    pub type Reply = WireReply;
    #[facade::message(harmos_path = ::facade::__harmos)]
    pub struct WireReply { pub value: String }
}
fn main() {}
