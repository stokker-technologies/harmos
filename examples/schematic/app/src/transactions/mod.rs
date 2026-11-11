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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Transactions {
    PlaceSymbol(symbol::PlaceSymbol),
    MoveSymbol(symbol::MoveSymbol),
    ConnectPins(net::ConnectPins),
    RenameNet(net::RenameNet),
    ApplyRoute(net::ApplyRoute),
}
impl From<symbol::PlaceSymbol> for Transactions {
    fn from(value: symbol::PlaceSymbol) -> Self {
        Self::PlaceSymbol(value)
    }
}
impl From<symbol::MoveSymbol> for Transactions {
    fn from(value: symbol::MoveSymbol) -> Self {
        Self::MoveSymbol(value)
    }
}
impl From<net::ConnectPins> for Transactions {
    fn from(value: net::ConnectPins) -> Self {
        Self::ConnectPins(value)
    }
}
impl From<net::RenameNet> for Transactions {
    fn from(value: net::RenameNet) -> Self {
        Self::RenameNet(value)
    }
}
impl From<net::ApplyRoute> for Transactions {
    fn from(value: net::ApplyRoute) -> Self {
        Self::ApplyRoute(value)
    }
}
impl harmos_runtime::state::Transaction<Schematic> for Transactions {
    type Error = SchematicError;
    fn check(&self, state: &Schematic) -> std::result::Result<(), Self::Error> {
        match self {
            Self::PlaceSymbol(value) => harmos_runtime::state::Transaction::check(value, state).map_err(Into::into),
            Self::MoveSymbol(value) => harmos_runtime::state::Transaction::check(value, state).map_err(Into::into),
            Self::ConnectPins(value) => harmos_runtime::state::Transaction::check(value, state).map_err(Into::into),
            Self::RenameNet(value) => harmos_runtime::state::Transaction::check(value, state).map_err(Into::into),
            Self::ApplyRoute(value) => harmos_runtime::state::Transaction::check(value, state).map_err(Into::into),
        }
    }
    fn apply(&self, state: &mut Schematic) {
        match self {
            Self::PlaceSymbol(value) => harmos_runtime::state::Transaction::apply(value, state),
            Self::MoveSymbol(value) => harmos_runtime::state::Transaction::apply(value, state),
            Self::ConnectPins(value) => harmos_runtime::state::Transaction::apply(value, state),
            Self::RenameNet(value) => harmos_runtime::state::Transaction::apply(value, state),
            Self::ApplyRoute(value) => harmos_runtime::state::Transaction::apply(value, state),
        }
    }
    fn records(&self, state: &Schematic, records: &mut harmos_runtime::state::Records) {
        match self {
            Self::PlaceSymbol(value) => harmos_runtime::state::Transaction::records(value, state, records),
            Self::MoveSymbol(value) => harmos_runtime::state::Transaction::records(value, state, records),
            Self::ConnectPins(value) => harmos_runtime::state::Transaction::records(value, state, records),
            Self::RenameNet(value) => harmos_runtime::state::Transaction::records(value, state, records),
            Self::ApplyRoute(value) => harmos_runtime::state::Transaction::records(value, state, records),
        }
    }
}
impl harmos_runtime::state::CatalogCodec for Transactions {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        match self {
            Self::PlaceSymbol(_) => symbol::PlaceSymbol::ENTRY.definition(),
            Self::MoveSymbol(_) => symbol::MoveSymbol::ENTRY.definition(),
            Self::ConnectPins(_) => net::ConnectPins::ENTRY.definition(),
            Self::RenameNet(_) => net::RenameNet::ENTRY.definition(),
            Self::ApplyRoute(_) => net::ApplyRoute::ENTRY.definition(),
        }
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::PlaceSymbol(value) => serde::Serialize::serialize(value, serializer),
            Self::MoveSymbol(value) => serde::Serialize::serialize(value, serializer),
            Self::ConnectPins(value) => serde::Serialize::serialize(value, serializer),
            Self::RenameNet(value) => serde::Serialize::serialize(value, serializer),
            Self::ApplyRoute(value) => serde::Serialize::serialize(value, serializer),
        }
    }
}
pub fn transactions_state_catalog() -> harmos_runtime::state::StateCatalog<Schematic, Transactions> {
    harmos_runtime::state::StateCatalog::new()
        .transaction(symbol::PlaceSymbol::ENTRY, Transactions::PlaceSymbol, |value| match value {
            Transactions::PlaceSymbol(value) => Some(value),
            _ => None,
        })
        .transaction(symbol::MoveSymbol::ENTRY, Transactions::MoveSymbol, |value| match value {
            Transactions::MoveSymbol(value) => Some(value),
            _ => None,
        })
        .transaction(net::ConnectPins::ENTRY, Transactions::ConnectPins, |value| match value {
            Transactions::ConnectPins(value) => Some(value),
            _ => None,
        })
        .transaction(net::RenameNet::ENTRY, Transactions::RenameNet, |value| match value {
            Transactions::RenameNet(value) => Some(value),
            _ => None,
        })
        .transaction(net::ApplyRoute::ENTRY, Transactions::ApplyRoute, |value| match value {
            Transactions::ApplyRoute(value) => Some(value),
            _ => None,
        })
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
