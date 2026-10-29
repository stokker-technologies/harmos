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

// Shared ownership has no message wire meaning; only `Box<T>` is admitted.
#[harmos::message]
struct Holder {
    row: std::sync::Arc<Leaf>,
}


#[harmos::message]
enum Shared {
    Owned(std::rc::Rc<Leaf>),
}


fn main() {}
