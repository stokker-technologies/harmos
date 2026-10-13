/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub mod net;
pub mod symbol;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use crate::error::SchematicError;
use crate::state::{Schematic, SymbolId};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[harmos::transactions(state = Schematic, error = SchematicError)]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Transactions {
    PlaceSymbol(symbol::PlaceSymbol),
    MoveSymbol(symbol::MoveSymbol),
    ConnectPins(net::ConnectPins),
    RenameNet(net::RenameNet),
    ApplyRoute(net::ApplyRoute),
}

/* === Traits === */

/* === Implementations === */
impl Transactions {
    pub fn symbol(&self) -> Option<SymbolId> {
        match self {
            Self::PlaceSymbol(change) => Some(change.symbol),
            Self::MoveSymbol(change) => Some(change.symbol),
            Self::ConnectPins(_) | Self::RenameNet(_) | Self::ApplyRoute(_) => None,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
