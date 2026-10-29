/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use serde::{Deserialize, Serialize};

use crate::error::SchematicError;
use crate::state::{Point, Rotation, Schematic, Symbol, SymbolId};
use harmos::state::Transaction;

#[harmos::transaction(id = "symbol.place")]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct PlaceSymbol {
    pub symbol: SymbolId,
    pub reference: String,
    pub at: Point,
    pub rotation: Rotation,
}

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
