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
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::window::Window;
use crate::Error;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct Demand {
    updates: watch::Receiver<usize>,
}

pub(crate) struct Holding {
    window: Arc<Window>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Demand {
    pub(crate) fn of(window: &Window) -> Self {
        Self { updates: window.interest() }
    }

    pub async fn changed(&mut self) -> Result<(), Error> {
        self.updates.changed().await.map_err(|_| Error::Unavailable)
    }

    pub fn borrow(&self) -> impl Deref<Target = usize> + '_ {
        self.updates.borrow()
    }
}

impl Deref for Demand {
    type Target = watch::Receiver<usize>;

    fn deref(&self) -> &Self::Target {
        &self.updates
    }
}

impl DerefMut for Demand {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.updates
    }
}

impl Holding {
    pub(crate) fn engage(window: Arc<Window>) -> Self {
        window.engaged();
        Self { window }
    }
}

impl Drop for Holding {
    fn drop(&mut self) {
        self.window.released();
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
