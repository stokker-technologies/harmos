/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Settings Calls

/* === Modules === */

/* === Re-Exports === */

pub use harmos_protocol::host::plugins::{SettingsListRequest, SettingsListResponse, SettingsSetRequest, SettingsSetResponse};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::Call;
use crate::Result;
use harmos_operations::host::declarations::settings;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Settings<'a> {
    pub(super) call: &'a Call,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Settings<'_> {
    pub async fn list(&self, request: SettingsListRequest) -> Result<SettingsListResponse> {
        self.call.call::<settings::List>(request).await
    }

    pub async fn set(&self, request: SettingsSetRequest) -> Result<SettingsSetResponse> {
        self.call.call::<settings::Set>(request).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
