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

// A box owns a nested message, never a container or bytes.
#[harmos::message]
struct Holder {
    rows: Box<Vec<Leaf>>,
}


fn main() {}
