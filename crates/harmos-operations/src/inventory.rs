/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod contracts;
pub mod listeners;
pub mod observers;
pub mod operations;
pub mod publications;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::Deserialize;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamDeclaration {
    pub metadata: harmos_domain::Metadata,

    #[serde(default)]
    pub row_metadata: Vec<harmos_domain::StreamField>,

    pub id: String,
    pub window: usize,
    pub row: harmos_domain::schema::graph::Schema,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
