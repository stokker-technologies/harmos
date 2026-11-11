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
use crate::state::{Point, Rotation, Schematic, Symbol, SymbolId};
use harmos_runtime::state::Transaction;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_runtime::transaction(id = "symbol.place")]
#[derive(harmos_runtime::Described, Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct PlaceSymbol {
    pub symbol: SymbolId,
    pub reference: String,
    pub at: Point,
    pub rotation: Rotation,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Transaction<Schematic> for PlaceSymbol {
    type Error = SchematicError;

    fn check(&self, schematic: &Schematic) -> Result<(), Self::Error> {
        schematic.require_symbol_free(self.symbol)
    }

    fn apply(&self, schematic: &mut Schematic) {
        schematic.symbols.insert(
            self.symbol,
            Symbol {
                reference: self.reference.clone(),
                position: self.at,
                rotation: self.rotation,
            },
        );
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
