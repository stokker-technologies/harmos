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
use crate::state::{NetId, NetName, Schematic};
use harmos_runtime::{state::Invertible, state::Transaction};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_runtime::Described, Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct RenameNet {
    pub net: NetId,
    pub to: NetName,
}
impl RenameNet {
    pub const ENTRY: harmos_runtime::state::TransactionEntry<Schematic, Self> =
        harmos_runtime::state::TransactionEntry::new("net.rename").reversible::<Self>();
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Transaction<Schematic> for RenameNet {
    type Error = SchematicError;

    fn check(&self, schematic: &Schematic) -> Result<(), Self::Error> {
        schematic.require_net(self.net)?;
        schematic.require_name_free(&self.to)
    }

    fn apply(&self, schematic: &mut Schematic) {
        schematic.net_mut(self.net).name = self.to.clone();
    }
}

impl Invertible<Schematic> for RenameNet {
    type Inverse = Self;

    fn inverse(&self, before: &Schematic) -> Self::Inverse {
        Self {
            net: self.net,
            to: before.net(self.net).name.clone(),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
