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
use crate::state::{NetId, Point, Schematic};
use harmos::{state::Invertible, state::Transaction};

#[harmos::transaction(id = "net.route",  inverse = Self)]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ApplyRoute {
    pub net: NetId,
    pub points: Vec<Point>,
}

impl Transaction<Schematic> for ApplyRoute {
    type Error = SchematicError;

    fn check(&self, schematic: &Schematic) -> Result<(), Self::Error> {
        schematic.require_net(self.net)?;
        if self.points.len() == 1 || self.points.windows(2).any(|pair| pair[0].x != pair[1].x && pair[0].y != pair[1].y) {
            return Err(SchematicError::InvalidRoute);
        }
        Ok(())
    }

    fn apply(&self, schematic: &mut Schematic) {
        schematic.net_mut(self.net).route = self.points.clone();
    }
}

impl Invertible<Schematic> for ApplyRoute {
    type Inverse = Self;

    fn inverse(&self, before: &Schematic) -> Self::Inverse {
        Self {
            net: self.net,
            points: before.net(self.net).route.clone(),
        }
    }
}
