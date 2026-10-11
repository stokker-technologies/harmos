/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod history {
    use harmos::Transaction;
    use serde::Deserialize;

    use super::{NetId, NetName, Schematic, SchematicError};

    #[harmos::transaction(version = 1)]
    #[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
    pub(super) struct RenameNetV1 {
        pub(super) net: NetId,
        pub(super) to: String,
    }

    impl Transaction<Schematic> for RenameNetV1 {
        type Error = SchematicError;

        fn check(&self, schematic: &Schematic) -> Result<(), Self::Error> {
            schematic.require_net(self.net)
        }

        fn apply(&self, schematic: &mut Schematic) {
            schematic.net_mut(self.net).name = NetName::verbatim(&self.to);
        }
    }
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::error::SchematicError;
use crate::state::{NetId, NetName, Schematic};
use harmos::{Invert, Transaction};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::transaction(
    id = "net.rename",
    version = 2,
    inverse = Self,
    history(history::RenameNetV1),
)]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct RenameNet {
    pub net: NetId,
    pub to: NetName,
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

impl Invert<Schematic> for RenameNet {
    fn invert(&self, before: &Schematic) -> Self {
        Self {
            net: self.net,
            to: before.net(self.net).name.clone(),
        }
    }
}

impl From<history::RenameNetV1> for RenameNet {
    fn from(old: history::RenameNetV1) -> Self {
        Self {
            net: old.net,
            to: NetName::verbatim(old.to),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
