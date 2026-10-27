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

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Sheet;

#[harmos::transaction(id = "report.send",  irreversible)]
#[derive(Deserialize, Serialize)]
struct SendReport;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos::state::Transaction<Sheet> for SendReport {
    type Error = ();

    fn check(&self, _sheet: &Sheet) -> Result<(), Self::Error> {
        Ok(())
    }

    fn apply(&self, _sheet: &mut Sheet) {}
}

impl harmos::state::Invert<Sheet> for SendReport {
    fn invert(&self, _sheet: &Sheet) -> SendReport {
        SendReport
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
