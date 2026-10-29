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
use crate::state::{Point, Schematic, SymbolId};
use harmos::{state::Invertible, state::Transaction};

#[harmos::transaction(id = "symbol.move",  inverse = Self)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct MoveSymbol {
    pub symbol: SymbolId,
    pub to: Point,
}

impl Transaction<Schematic> for MoveSymbol {
    type Error = SchematicError;

    fn check(&self, schematic: &Schematic) -> Result<(), Self::Error> {
        schematic.require_symbol(self.symbol)
    }

    fn apply(&self, schematic: &mut Schematic) {
        schematic.symbol_mut(self.symbol).position = self.to;
    }
}

impl Invertible<Schematic> for MoveSymbol {
    type Inverse = Self;

    fn inverse(&self, before: &Schematic) -> Self::Inverse {
        Self {
            symbol: self.symbol,
            to: before.symbol(self.symbol).position,
        }
    }
}
