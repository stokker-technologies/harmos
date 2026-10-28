/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


#[harmos::stream(id = "pulse", window = 8)]
struct FastPulse;

#[harmos::stream(id = "pulse", window = 4)]
struct SlowPulse;


fn main() {}
