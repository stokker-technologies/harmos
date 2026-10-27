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
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use crate::error::SchematicError;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SymbolId(pub u64);

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct NetId(pub u64);

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct PinRef {
    pub symbol: SymbolId,
    pub pin: u32,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct NetName {
    pub base: String,
    pub member: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Symbol {
    pub reference: String,
    pub position: Point,
    pub rotation: Rotation,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Net {
    pub name: NetName,
    pub pins: BTreeSet<PinRef>,

    #[serde(default)]
    pub route: Vec<Point>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Schematic {
    pub symbols: BTreeMap<SymbolId, Symbol>,
    pub nets: BTreeMap<NetId, Net>,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum Rotation {
    #[default]
    Zero,

    Quarter,
    Half,
    ThreeQuarter,
}

/* === Traits === */

/* === Implementations === */

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { x, y } = self;

        write!(formatter, "({x}, {y})")
    }
}

impl Rotation {
    pub fn quarters(turns: u8) -> Self {
        match turns % 4 {
            1 => Self::Quarter,
            2 => Self::Half,
            3 => Self::ThreeQuarter,
            _ => Self::Zero,
        }
    }
}

impl PinRef {
    pub const fn new(symbol: SymbolId, pin: u32) -> Self {
        Self { symbol, pin }
    }
}

impl fmt::Display for PinRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { symbol, pin } = self;

        write!(formatter, "#{}.{pin}", symbol.0)
    }
}

impl NetName {
    pub fn parse(text: &str) -> Self {
        let Some((base, member)) = text.strip_suffix(']').and_then(|open| open.rsplit_once('[')) else {
            return Self::verbatim(text);
        };

        member.parse().map_or_else(
            |_| Self::verbatim(text),
            |member| Self {
                base: base.to_owned(),
                member: Some(member),
            },
        )
    }

    pub fn verbatim(text: impl Into<String>) -> Self {
        Self { base: text.into(), member: None }
    }
}

impl fmt::Display for NetName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.base)?;
        match self.member {
            Some(member) => write!(formatter, "[{member}]"),
            None => Ok(()),
        }
    }
}

impl Schematic {
    pub fn require_symbol(&self, symbol: SymbolId) -> Result<(), SchematicError> {
        self.symbols.contains_key(&symbol).then_some(()).ok_or(SchematicError::UnknownSymbol(symbol))
    }

    pub fn require_symbol_free(&self, symbol: SymbolId) -> Result<(), SchematicError> {
        (!self.symbols.contains_key(&symbol)).then_some(()).ok_or(SchematicError::SymbolTaken(symbol))
    }

    pub fn require_net(&self, net: NetId) -> Result<(), SchematicError> {
        self.nets.contains_key(&net).then_some(()).ok_or(SchematicError::UnknownNet(net))
    }

    pub fn require_name_free(&self, name: &NetName) -> Result<(), SchematicError> {
        self.nets.values().all(|net| net.name != *name).then_some(()).ok_or_else(|| SchematicError::NameTaken(name.to_string()))
    }

    pub fn symbol(&self, symbol: SymbolId) -> &Symbol {
        self.symbols.get(&symbol).expect("check refuses a change naming a symbol that is not placed")
    }

    pub fn symbol_mut(&mut self, symbol: SymbolId) -> &mut Symbol {
        self.symbols.get_mut(&symbol).expect("check refuses a change naming a symbol that is not placed")
    }

    pub fn net(&self, net: NetId) -> &Net {
        self.nets.get(&net).expect("check refuses a change naming a net that does not exist")
    }

    pub fn net_mut(&mut self, net: NetId) -> &mut Net {
        self.nets.get_mut(&net).expect("check refuses a change naming a net that does not exist")
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
