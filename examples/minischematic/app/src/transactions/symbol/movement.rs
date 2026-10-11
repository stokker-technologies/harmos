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
use crate::error::SchematicError;
use crate::state::{Point, Schematic, SymbolId};
use harmos::{Invert, Transaction};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::transaction(id = "symbol.move", version = 1, inverse = Self)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct MoveSymbol {
    pub symbol: SymbolId,
    pub to: Point,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Transaction<Schematic> for MoveSymbol {
    type Error = SchematicError;

    fn check(&self, schematic: &Schematic) -> Result<(), Self::Error> {
        schematic.require_symbol(self.symbol)
    }

    fn apply(&self, schematic: &mut Schematic) {
        schematic.symbol_mut(self.symbol).position = self.to;
    }
}

impl Invert<Schematic> for MoveSymbol {
    fn invert(&self, before: &Schematic) -> Self {
        Self {
            symbol: self.symbol,
            to: before.symbol(self.symbol).position,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
