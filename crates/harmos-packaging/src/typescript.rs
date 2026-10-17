/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generate Typed Protobuf JSON Messages And Operation Metadata.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

// Third Party Imports
use prost_reflect::{DescriptorPool, Kind, MessageDescriptor};

// Local Imports
use crate::proto::{Artifacts, ExportError};
use harmos_domain::operations::Contract;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
// Values Follow Protobuf JSON: 64-Bit Integers And Bytes Are Strings.
pub fn generate(catalog: &[Contract], output: &Path) -> Result<PathBuf, ExportError> {
    let artifacts = Artifacts::new(catalog)?;
    let pool = DescriptorPool::decode(artifacts.descriptors())
        .map_err(crate::proto::DescriptorError::from)?;
    let mut names = BTreeMap::new();
    for message in pool.all_messages() {
        let name = message.full_name().replace('.', "_");
        if names.values().any(|previous| previous == &name) {
            return Err(ExportError::Collision(name));
        }
        names.insert(message.full_name().to_owned(), name);
    }
    let mut source = String::from("// Generated From Harmos Operation Contracts. Do Not Edit.\n\n");
    for message in pool.all_messages() {
        message_source(&message, &names, &mut source)?;
    }
    source.push_str("export const operations = ");
    source.push_str(
        std::str::from_utf8(artifacts.contracts()).map_err(|_| ExportError::Unsupported)?,
    );
    source.push_str(" as const;\n");
    if source.len() > 64 * 1024 * 1024 {
        return Err(ExportError::Limit);
    }
    std::fs::create_dir_all(output)?;
    let path = output.join("index.ts");
    std::fs::write(&path, source)?;
    std::fs::write(output.join("contracts.json"), artifacts.contracts())?;
    std::fs::write(output.join("descriptors.bin"), artifacts.descriptors())?;
    Ok(path)
}

/* === Private Functions === */
fn message_source(
    message: &MessageDescriptor,
    names: &BTreeMap<String, String>,
    source: &mut String,
) -> Result<(), ExportError> {
    let name = &names[message.full_name()];
    writeln!(source, "export type {name} = {{").expect("String Writer");
    for field in message.fields() {
        if field
            .containing_oneof()
            .is_some_and(|oneof| !oneof.is_synthetic())
        {
            continue;
        }
        let kind = field_type(field.kind(), names)?;
        // ProtoJSON May Omit Defaults, Including Empty Lists And Scalars.
        let optional = true;
        let field_name = serde_json::to_string(field.json_name())?;
        writeln!(
            source,
            "  {field_name}{}: {kind}{};",
            if optional { "?" } else { "" },
            if field.is_list() { "[]" } else { "" }
        )
        .expect("String Writer");
    }
    source.push('}');
    for oneof in message.oneofs().filter(|oneof| !oneof.is_synthetic()) {
        source.push_str(" & (\n");
        for selected in oneof.fields() {
            source.push_str("  | { ");
            for field in oneof.fields() {
                let name = serde_json::to_string(field.json_name())?;
                if field.number() == selected.number() {
                    let kind = field_type(field.kind(), names)?;
                    write!(source, "{name}: {kind}; ").expect("String Writer");
                } else {
                    write!(source, "{name}?: never; ").expect("String Writer");
                }
            }
            source.push_str("}\n");
        }
        source.push(')');
    }
    source.push_str(";\n\n");
    Ok(())
}

fn field_type(kind: Kind, names: &BTreeMap<String, String>) -> Result<String, ExportError> {
    let value = match kind {
        Kind::Bool => "boolean",
        Kind::Int32
        | Kind::Sint32
        | Kind::Sfixed32
        | Kind::Uint32
        | Kind::Fixed32
        | Kind::Float
        | Kind::Double => "number",
        Kind::Int64
        | Kind::Sint64
        | Kind::Sfixed64
        | Kind::Uint64
        | Kind::Fixed64
        | Kind::String
        | Kind::Bytes => "string",
        Kind::Message(message) => {
            return names
                .get(message.full_name())
                .cloned()
                .ok_or(ExportError::Unsupported);
        }
        Kind::Enum(_) => return Err(ExportError::Unsupported),
    };
    Ok(value.to_owned())
}

/* === Tests === */
