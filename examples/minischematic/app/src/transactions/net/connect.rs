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
use std::collections::BTreeSet;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::error::SchematicError;
use crate::state::{Net, NetId, NetName, PinRef, Schematic};
use harmos::Transaction;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::transaction(id = "net.connect", version = 1, irreversible)]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ConnectPins {
    pub net: NetId,
    pub name: NetName,
    pub pins: BTreeSet<PinRef>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Transaction<Schematic> for ConnectPins {
    type Error = SchematicError;

    fn check(&self, schematic: &Schematic) -> Result<(), Self::Error> {
        if self.pins.len() < 2 {
            return Err(SchematicError::TooFewPins);
        }
        for pin in &self.pins {
            schematic.require_symbol(pin.symbol)?;
        }
        if schematic.nets.contains_key(&self.net) {
            return Ok(());
        }

        schematic.require_name_free(&self.name)
    }

    fn apply(&self, schematic: &mut Schematic) {
        schematic
            .nets
            .entry(self.net)
            .or_insert_with(|| Net {
                name: self.name.clone(),
                pins: BTreeSet::new(),
            })
            .pins
            .extend(self.pins.iter().copied());
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
