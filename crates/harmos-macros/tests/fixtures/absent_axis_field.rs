/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


#[harmos::stream(id = "waveform", window = 8, axis = elapsed)]
struct Waveform {
    ordinal: u64,
}


fn main() {}
