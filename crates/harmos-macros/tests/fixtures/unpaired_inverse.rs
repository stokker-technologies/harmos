/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


use serde::{Deserialize, Serialize};


#[harmos::transaction(id = "sheet.place",  inverse = Remove)]
#[derive(Deserialize, Serialize)]
struct Place;

#[harmos::transaction(id = "sheet.remove",  inverse = Self)]
#[derive(Deserialize, Serialize)]
struct Remove;


fn main() {}
