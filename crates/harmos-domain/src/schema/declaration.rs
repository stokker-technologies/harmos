/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Static Shared Type Declarations And Inert Metadata

use std::any::TypeId;
use std::collections::HashMap;
use std::fmt;
use std::mem::size_of;

use serde::{Deserialize, Serialize};

use super::graph::{self, Error, Schema};
use super::{Cardinality, MAX_NAME_BYTES};
use crate::config::SchemaSettings;
use crate::inventory::{digits, number, quoted, quoted_len, write};

const HEADER: &[u8] = b"{\"kind\":\"schema\",\"name\":";
const FIELDS: &[u8] = b",\"fields\":[";
const NAME: &[u8] = b"{\"name\":";
const TAG: &[u8] = b",\"tag\":";
const KIND: &[u8] = b",\"kind\":{\"type\":";
const REFERENCE: &[u8] = b",\"definition\":";
const CARDINALITY: &[u8] = b"},\"cardinality\":";

pub type Field<R = Definition> = graph::Field<R>;
pub type Kind<R = Definition> = graph::Kind<R>;

#[derive(Clone, Copy)]
pub struct Definition {
    pub name: &'static str,

    pub default_variant: Option<usize>,

    pub identity: fn() -> TypeId,
    pub fields: fn() -> &'static [Field<Definition>],
    pub marker: fn() -> &'static [u8],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Fragment", into = "Fragment")]
pub struct Named {
    pub name: String,
    pub fields: Vec<Field<String>>,
}

// Bounded Static Storage For Names Of Generic Message Shapes.
pub struct Name {
    bytes: [u8; MAX_NAME_BYTES],
    length: usize,
}

struct Plan {
    declarations: Vec<Definition>,
    indices: HashMap<&'static str, usize>,
    remaining_bytes: usize,
    remaining_work: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Fragment {
    Schema { name: String, fields: Vec<Field<String>> },
}

impl PartialEq for Definition {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && (self.identity)() == (other.identity)()
    }
}
impl Eq for Definition {}

impl Name {
    pub const fn new(owner: &str, arguments: &[&str]) -> Self {
        let mut name = Self { bytes: [0; MAX_NAME_BYTES], length: 0 };
        name.push(owner);
        if !arguments.is_empty() {
            name.push("<");
            let mut index = 0;
            while index < arguments.len() {
                if index > 0 {
                    name.push(",");
                }
                name.push(arguments[index]);
                index += 1;
            }
            name.push(">");
        }
        name
    }

    pub const fn as_str(&self) -> &str {
        match std::str::from_utf8(self.bytes.split_at(self.length).0) {
            Ok(name) => name,
            Err(_) => panic!("schema names originate from valid UTF-8"),
        }
    }

    const fn push(&mut self, part: &str) {
        assert!(part.len() <= MAX_NAME_BYTES - self.length, "schema name exceeds its bound");
        let mut index = 0;
        while index < part.len() {
            self.bytes[self.length] = part.as_bytes()[index];
            self.length += 1;
            index += 1;
        }
    }
}

impl Definition {
    pub const fn of<T: super::Described + ?Sized>() -> Self {
        Self {
            name: T::NAME,
            default_variant: T::DEFAULT_VARIANT,
            identity: TypeId::of::<T>,
            fields: fields::<T>,
            marker: marker::<T>,
        }
    }
}

impl Named {
    pub fn decode<'a>(fragments: impl IntoIterator<Item = &'a [u8]>, settings: &SchemaSettings) -> Result<Vec<Self>, Error> {
        let mut budget = Plan::new(settings);
        let mut declarations = Vec::new();
        for encoded in fragments {
            if encoded.len() > crate::FRAGMENT_LIMIT {
                return Err(Error::Budget);
            }
            // Flat JSON Input Bounds Parser Allocations Before Decoding Each Type.
            budget.charge(encoded.len().checked_mul(8).ok_or(Error::Budget)? + 4 * size_of::<Self>(), encoded.len() + 1)?;
            let declaration: Self = serde_json::from_slice(encoded).map_err(|_| Error::Field {
                field: "schema fragment".into(),
                reason: "invalid schema metadata",
            })?;
            declarations.push(declaration);
        }
        Ok(declarations)
    }
}

impl From<Fragment> for Named {
    fn from(fragment: Fragment) -> Self {
        let Fragment::Schema { name, fields } = fragment;
        Self { name, fields }
    }
}
impl From<Named> for Fragment {
    fn from(named: Named) -> Self {
        Self::Schema { name: named.name, fields: named.fields }
    }
}

impl fmt::Debug for Definition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Definition").field("name", &self.name).finish_non_exhaustive()
    }
}

impl Schema {
    // Resolve Inert Named Metadata Without Executing Artifact Code.
    pub fn from_named(root: &str, declarations: &[Named], settings: &SchemaSettings) -> Result<Self, Error> {
        resolve_named(&[("", root)], declarations, settings, false)
    }

    // Resolve Multiple Contract Directions From One Inert Type Dictionary.
    pub fn from_named_roots(roots: &[(&str, &str)], declarations: &[Named], settings: &SchemaSettings) -> Result<Self, Error> {
        resolve_named(roots, declarations, settings, true)
    }

    pub fn from_declaration(root: Definition, settings: &SchemaSettings) -> Result<Self, Error> {
        let mut plan = Plan::new(settings);
        plan.insert(root)?;
        let schema = Self { root: 0, definitions: plan.compile()? };
        schema.prepare(settings)?;
        Ok(schema)
    }

    // Share Each Message Definition Across A Set Of Contract Directions.
    pub fn from_declarations(roots: &[(&str, Definition)], settings: &SchemaSettings) -> Result<Self, Error> {
        let mut plan = Plan::new(settings);
        plan.charge(4 * size_of::<Vec<Field>>() + size_of::<Self>(), 1)?;
        let mut fields = Vec::new();
        for (index, (name, declaration)) in roots.iter().enumerate() {
            plan.charge(4 * size_of::<Field<usize>>() + name.len(), name.len() + 1)?;
            let tag = u32::try_from(index + 1).map_err(|_| Error::Budget)?;
            fields.push(Field {
                name: (*name).to_owned().into(),
                tag,
                kind: Kind::Message { definition: plan.insert(*declaration)? },
                cardinality: Cardinality::Required,
            });
        }
        let mut definitions = plan.compile()?;
        let root = definitions.len();
        definitions.push(fields);
        let schema = Self { root, definitions };
        schema.prepare(settings)?;
        Ok(schema)
    }
}

impl Plan {
    fn compile(mut self) -> Result<Vec<Vec<Field<usize>>>, Error> {
        let mut definitions = Vec::new();
        let mut index = 0;
        while index < self.declarations.len() {
            let declaration = self.declarations[index];
            let mut fields = Vec::new();
            for field in (declaration.fields)() {
                self.charge(4 * size_of::<Field<usize>>() + field.name.len(), field.name.len() + 1)?;
                let kind = field.kind.try_map(|definition| self.insert(definition))?;
                fields.push(Field {
                    name: field.name.to_string().into(),
                    tag: field.tag,
                    kind,
                    cardinality: field.cardinality,
                });
            }
            definitions.push(fields);
            index += 1;
        }
        Ok(definitions)
    }
    fn new(settings: &SchemaSettings) -> Self {
        Self {
            declarations: Vec::new(),
            indices: HashMap::new(),
            remaining_bytes: settings.preparation_bytes.get(),
            remaining_work: settings.preparation_work.get(),
        }
    }
    fn charge(&mut self, bytes: usize, work: usize) -> Result<(), Error> {
        self.remaining_bytes = self.remaining_bytes.checked_sub(bytes).ok_or(Error::Budget)?;
        self.remaining_work = self.remaining_work.checked_sub(work).ok_or(Error::Budget)?;
        Ok(())
    }
    fn insert(&mut self, definition: Definition) -> Result<usize, Error> {
        self.charge(0, definition.name.len() + 1)?;
        if definition.name.is_empty() || definition.name.len() > MAX_NAME_BYTES || definition.name.chars().any(char::is_control) {
            return Err(Error::Field {
                field: super::diagnostic_name(definition.name),
                reason: "invalid schema definition name",
            });
        }
        if let Some(index) = self.indices.get(definition.name).copied() {
            let original = self.declarations[index];
            if (original.identity)() != (definition.identity)() {
                return Err(Error::Field {
                    field: definition.name.into(),
                    reason: "schema definition name has multiple owners",
                });
            }
            return Ok(index);
        }
        self.charge(4 * (size_of::<Definition>() + size_of::<Vec<Field>>() + size_of::<(&str, usize)>() + 1), 1)?;
        let index = self.declarations.len();
        self.declarations.push(definition);
        self.indices.insert(definition.name, index);
        Ok(index)
    }
}

pub fn markers(root: Definition, settings: &SchemaSettings) -> Result<Vec<&'static [u8]>, Error> {
    markers_many([root], settings)
}

pub fn markers_many(roots: impl IntoIterator<Item = Definition>, settings: &SchemaSettings) -> Result<Vec<&'static [u8]>, Error> {
    let mut plan = Plan::new(settings);
    for root in roots {
        plan.insert(root)?;
    }
    let mut index = 0;
    while index < plan.declarations.len() {
        let declaration = plan.declarations[index];
        for field in (declaration.fields)() {
            plan.charge(0, 1)?;
            if let Kind::Message { definition } = field.kind {
                plan.insert(definition)?;
            }
        }
        index += 1;
    }
    let output_bytes = plan.declarations.len().checked_mul(std::mem::size_of::<&[u8]>()).ok_or(Error::Budget)?;
    plan.charge(output_bytes, 0)?;
    Ok(plan.declarations.iter().map(|definition| (definition.marker)()).collect())
}

pub const fn encoded_len(name: &str, fields: &[Field<Definition>]) -> usize {
    let mut length = crate::PREFIX.len() + HEADER.len() + quoted_len(name) + FIELDS.len() + 3;
    let mut index = 0;
    while index < fields.len() {
        let field = &fields[index];
        length += NAME.len()
            + quoted_len(field_name(field))
            + TAG.len()
            + digits(field.tag as u64)
            + KIND.len()
            + quoted_len(kind_name(&field.kind))
            + CARDINALITY.len()
            + quoted_len(cardinality_name(field.cardinality))
            + 1;
        if let Kind::Message { definition } = &field.kind {
            length += REFERENCE.len() + quoted_len(definition.name);
        }
        if index > 0 {
            length += 1;
        }
        index += 1;
    }
    assert!(length - crate::PREFIX.len() - 1 <= crate::FRAGMENT_LIMIT);
    length
}

pub const fn encode<const N: usize>(name: &str, fields: &[Field<Definition>]) -> [u8; N] {
    assert!(N == encoded_len(name, fields));
    encode_buffer(name, fields)
}

// Encode A Small Generic Shape Into Explicitly Bounded Static Storage.
// The Caller Retains Only The `encoded_len` Prefix Of The Result.
pub const fn encode_buffer<const N: usize>(name: &str, fields: &[Field<Definition>]) -> [u8; N] {
    assert!(N >= encoded_len(name, fields), "schema marker exceeds its static storage");
    let mut output = [0; N];
    let mut cursor = 0;
    write(&mut output, &mut cursor, crate::PREFIX);
    write(&mut output, &mut cursor, HEADER);
    quoted(&mut output, &mut cursor, name);
    write(&mut output, &mut cursor, FIELDS);
    let mut index = 0;
    while index < fields.len() {
        let field = &fields[index];
        if index > 0 {
            write(&mut output, &mut cursor, b",");
        }
        write(&mut output, &mut cursor, NAME);
        quoted(&mut output, &mut cursor, field_name(field));
        write(&mut output, &mut cursor, TAG);
        number(&mut output, &mut cursor, field.tag as u64);
        write(&mut output, &mut cursor, KIND);
        quoted(&mut output, &mut cursor, kind_name(&field.kind));
        if let Kind::Message { definition } = &field.kind {
            write(&mut output, &mut cursor, REFERENCE);
            quoted(&mut output, &mut cursor, definition.name);
        }
        write(&mut output, &mut cursor, CARDINALITY);
        quoted(&mut output, &mut cursor, cardinality_name(field.cardinality));
        write(&mut output, &mut cursor, b"}");
        index += 1;
    }
    write(&mut output, &mut cursor, b"]}\0");
    assert!(cursor == encoded_len(name, fields));
    output
}

fn fields<T: super::Described + ?Sized>() -> &'static [Field<Definition>] {
    T::FIELDS
}

fn marker<T: super::Described + ?Sized>() -> &'static [u8] {
    T::MARKER
}

fn resolve_named(roots: &[(&str, &str)], declarations: &[Named], settings: &SchemaSettings, composite: bool) -> Result<Schema, Error> {
    let mut budget = Plan::new(settings);
    let mut dictionary = HashMap::new();
    for declaration in declarations {
        let name = declaration.name.as_str();
        budget.charge(4 * size_of::<(&str, &Named)>() + name.len(), name.len() + 1)?;
        if name.is_empty() || name.len() > MAX_NAME_BYTES || name.chars().any(char::is_control) {
            return Err(Error::Field {
                field: super::diagnostic_name(name),
                reason: "invalid schema definition name",
            });
        }
        // Charge The Already Decoded Input Alongside Output And Lookup Scratch.
        budget.charge(declaration.fields.capacity() * size_of::<Field<String>>(), 1)?;
        for field in &declaration.fields {
            budget.charge(field.name.len(), field.name.len() + 1)?;
            if let Kind::Message { definition } = &field.kind {
                budget.charge(definition.len(), definition.len() + 1)?;
            }
        }
        if let Some(original) = dictionary.insert(name, declaration) {
            budget.charge(0, declaration.fields.len() + 1)?;
            if original != declaration {
                return Err(Error::Field {
                    field: name.into(),
                    reason: "schema definition name has conflicting metadata",
                });
            }
        }
    }
    let missing = |name: &str| Error::Field {
        field: super::diagnostic_name(name),
        reason: "schema definition is absent",
    };
    let mut names = Vec::new();
    let mut indices = HashMap::new();
    let mut directions = Vec::new();
    budget.charge(4 * size_of::<Vec<Field>>() + size_of::<Schema>(), 1)?;
    for (index, (direction, root)) in roots.iter().enumerate() {
        let declaration = dictionary.get(root).ok_or_else(|| missing(root))?;
        let name = declaration.name.as_str();
        let definition = if let Some(index) = indices.get(name) {
            *index
        } else {
            budget.charge(4 * (size_of::<&str>() + size_of::<(&str, usize)>()), name.len() + 1)?;
            let index = names.len();
            names.push(name);
            indices.insert(name, index);
            index
        };
        if composite {
            budget.charge(4 * size_of::<Field<usize>>() + direction.len(), direction.len() + 1)?;
            directions.push(Field {
                name: (*direction).to_owned().into(),
                tag: u32::try_from(index + 1).map_err(|_| Error::Budget)?,
                kind: Kind::Message { definition },
                cardinality: Cardinality::Required,
            });
        }
    }
    let mut definitions = Vec::new();
    let mut index = 0;
    while index < names.len() {
        let declaration = dictionary[names[index]];
        let mut fields = Vec::new();
        budget.charge(4 * (size_of::<&str>() + size_of::<(&str, usize)>() + size_of::<Vec<Field>>()), 1)?;
        for field in &declaration.fields {
            budget.charge(4 * size_of::<Field<usize>>() + field.name.len(), field.name.len() + 1)?;
            let kind = field.kind.clone().try_map(|name| {
                let declaration = dictionary.get(name.as_str()).ok_or_else(|| missing(&name))?;
                let name = declaration.name.as_str();
                if let Some(index) = indices.get(name) {
                    return Ok(*index);
                }
                budget.charge(4 * (size_of::<&str>() + size_of::<(&str, usize)>()), name.len() + 1)?;
                let index = names.len();
                indices.insert(name, index);
                names.push(name);
                Ok(index)
            })?;
            fields.push(Field {
                name: field.name.clone(),
                tag: field.tag,
                kind,
                cardinality: field.cardinality,
            });
        }
        definitions.push(fields);
        index += 1;
    }
    let root = if composite {
        let root = definitions.len();
        definitions.push(directions);
        root
    } else {
        0
    };
    let schema = Schema { root, definitions };
    schema.prepare(settings)?;
    Ok(schema)
}

const fn field_name(field: &Field<Definition>) -> &str {
    match &field.name {
        std::borrow::Cow::Borrowed(name) => name,
        std::borrow::Cow::Owned(_) => panic!("constant schema names must be borrowed"),
    }
}
const fn kind_name(kind: &Kind<Definition>) -> &'static str {
    match kind {
        Kind::Bool => "bool",
        Kind::U32 => "u32",
        Kind::U64 => "u64",
        Kind::I32 => "i32",
        Kind::I64 => "i64",
        Kind::F32 => "f32",
        Kind::F64 => "f64",
        Kind::String => "string",
        Kind::Bytes => "bytes",
        Kind::Message { .. } => "message",
    }
}
const fn cardinality_name(cardinality: Cardinality) -> &'static str {
    match cardinality {
        Cardinality::Required => "required",
        Cardinality::Optional => "optional",
        Cardinality::Repeated => "repeated",
        Cardinality::Variant => "variant",
    }
}
