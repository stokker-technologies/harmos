/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod progress;
pub mod validated;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Default, Clone, Debug, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(harmos_domain::Message, Default, Clone, Debug, PartialEq)]
pub struct Connection {
    pub from: u64,
    pub to: u64,
}

#[derive(harmos_domain::Message, Default, Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub symbols: Vec<u64>,
    pub connections: Vec<Connection>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Snapshot {
    pub fn diagnostics(&self) -> Vec<String> {
        let symbols: std::collections::BTreeSet<_> = self.symbols.iter().copied().collect();
        let mut diagnostics = Vec::new();
        if symbols.len() != self.symbols.len() {
            diagnostics.push("Duplicate Symbol IDs".to_owned());
        }
        for connection in &self.connections {
            let from_present = symbols.contains(&connection.from);
            let to_present = symbols.contains(&connection.to);
            if !(from_present && to_present) {
                diagnostics.push(format!("Connection {} to {} References a Missing Symbol", connection.from, connection.to));
            }
        }
        diagnostics
    }

    pub fn validate(&self) -> crate::Result<()> {
        if self.diagnostics().is_empty() { Ok(()) } else { Err(crate::Error::InvalidSchematic) }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

impl From<Snapshot> for schematic_protocol::Snapshot {
    fn from(value: Snapshot) -> Self {
        Self {
            symbols: value.symbols,
            connections: value.connections.into_iter().map(|c| schematic_protocol::Connection { from: c.from, to: c.to }).collect(),
        }
    }
}
impl From<schematic_protocol::Snapshot> for Snapshot {
    fn from(value: schematic_protocol::Snapshot) -> Self {
        Self {
            symbols: value.symbols,
            connections: value.connections.into_iter().map(|c| Connection { from: c.from, to: c.to }).collect(),
        }
    }
}
