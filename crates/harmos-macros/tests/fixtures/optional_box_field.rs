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

// An optional message stays `Option<T>`; boxing inside the option is refused.
#[harmos::message]
struct Holder {
    row: Option<Box<Leaf>>,
}


fn main() {}
