/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[derive(facade::Message, Debug, Default)]
#[harmos_path(::facade::__harmos)]
#[derive(facade::Request, Clone)]

#[request(response = relative::Reply)]
pub struct Echo { pub value: String }
mod relative {
    pub type Reply = WireReply;
    #[derive(facade::Message, Clone, Debug, Default)]
#[harmos_path(::facade::__harmos)]
    pub struct WireReply { pub value: String }
}
fn main() {}
