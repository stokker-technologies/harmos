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

// Repeated messages already live on the heap inside their vector.
#[harmos::message]
struct Holder {
    rows: Vec<Box<Leaf>>,
}


fn main() {}
