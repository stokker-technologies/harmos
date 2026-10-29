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

// A box owns a nested message; a boxed scalar has no message to own.
#[harmos::message]
struct Holder {
    row: Box<u64>,
}


fn main() {}
