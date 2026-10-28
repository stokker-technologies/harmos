/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


#[harmos::message]
struct Leaf {
    value: u64,
}

// One box already moves the message to the heap; a second adds nothing.
#[harmos::message]
struct Holder {
    row: Box<Box<Leaf>>,
}


fn main() {}
