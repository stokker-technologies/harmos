/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub trait Answer {
    type Ok;
    type Err;
}

impl<T, E> Answer for Result<T, E> {
    type Ok = T;
    type Err = E;
}
