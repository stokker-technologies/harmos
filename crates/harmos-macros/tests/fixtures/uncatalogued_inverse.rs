/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::convert::Infallible;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Sheet;

#[harmos::transaction(id = "sheet.place", version = 1, inverse = Remove)]
#[derive(Deserialize, Serialize)]
struct Place;

#[harmos::transaction(id = "sheet.remove", version = 1, inverse = Place)]
#[derive(Deserialize, Serialize)]
struct Remove;

/* === Enums === */
#[harmos::transactions(state = Sheet)]
enum Change {
    Place(Place),
}

/* === Traits === */

/* === Implementations === */
impl harmos::Transaction<Sheet> for Place {
    type Error = Infallible;

    fn check(&self, _sheet: &Sheet) -> Result<(), Self::Error> {
        Ok(())
    }

    fn apply(&self, _sheet: &mut Sheet) {}
}

impl harmos::Invert<Sheet> for Place {
    fn invert(&self, _sheet: &Sheet) -> Remove {
        Remove
    }
}

impl harmos::Transaction<Sheet> for Remove {
    type Error = Infallible;

    fn check(&self, _sheet: &Sheet) -> Result<(), Self::Error> {
        Ok(())
    }

    fn apply(&self, _sheet: &mut Sheet) {}
}

impl harmos::Invert<Sheet> for Remove {
    fn invert(&self, _sheet: &Sheet) -> Place {
        Place
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn main() {}

/* === Tests === */
