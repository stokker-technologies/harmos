/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub mod net;
pub mod symbol;

use serde::{Deserialize, Serialize};

use crate::error::SchematicError;
use crate::state::{Schematic, SymbolId};

#[harmos::transactions(state = Schematic, error = SchematicError)]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Transactions {
    PlaceSymbol(symbol::PlaceSymbol),
    MoveSymbol(symbol::MoveSymbol),
    ConnectPins(net::ConnectPins),
    RenameNet(net::RenameNet),
    ApplyRoute(net::ApplyRoute),
}

impl Transactions {
    pub fn symbol(&self) -> Option<SymbolId> {
        match self {
            Self::PlaceSymbol(change) => Some(change.symbol),
            Self::MoveSymbol(change) => Some(change.symbol),
            Self::ConnectPins(_) | Self::RenameNet(_) | Self::ApplyRoute(_) => None,
        }
    }
}
