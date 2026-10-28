/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


use std::convert::Infallible;

use serde::{Deserialize, Serialize};


struct Sheet;

#[harmos::transaction(id = "sheet.place",  inverse = Remove)]
#[derive(Deserialize, Serialize)]
struct Place;

#[harmos::transaction(id = "sheet.remove",  inverse = Place)]
#[derive(Deserialize, Serialize)]
struct Remove;


#[harmos::transactions(state = Sheet)]
enum Change {
    Place(Place),
}


impl harmos::state::Transaction<Sheet> for Place {
    type Error = Infallible;

    fn check(&self, _sheet: &Sheet) -> Result<(), Self::Error> {
        Ok(())
    }

    fn apply(&self, _sheet: &mut Sheet) {}
}

impl harmos::state::Invertible<Sheet> for Place {
    type Inverse = Remove;

    fn inverse(&self, _sheet: &Sheet) -> Self::Inverse {
        Remove
    }
}

impl harmos::state::Transaction<Sheet> for Remove {
    type Error = Infallible;

    fn check(&self, _sheet: &Sheet) -> Result<(), Self::Error> {
        Ok(())
    }

    fn apply(&self, _sheet: &mut Sheet) {}
}

impl harmos::state::Invertible<Sheet> for Remove {
    type Inverse = Place;

    fn inverse(&self, _sheet: &Sheet) -> Self::Inverse {
        Place
    }
}


fn main() {}
