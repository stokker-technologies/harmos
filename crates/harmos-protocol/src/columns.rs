/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Interval Window Columns and Stored Batch Rows Derived From a Stream Row

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost_reflect::{Kind, MessageDescriptor};
use prost_types::{
    DescriptorProto, FieldDescriptorProto, FileDescriptorProto,
    field_descriptor_proto::{Label, Type},
};

// Local Imports

/* === Constants === */

/// Field Numbers of the Stored Batch Row; Its Columns Keep the Row's Own Numbers.
pub mod batch {
    pub const START_UNIX_NANOS: u32 = 1;
    pub const PERIOD_NANOS: u32 = 2;
    pub const COUNT: u32 = 3;
    pub const COLUMNS: u32 = 4;
}

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// The Columns of an Interval Row and the Stored Batch Row That Holds Them.
#[derive(Clone, Debug)]
pub struct Derived {
    pub columns: MessageDescriptor,
    pub batch: MessageDescriptor,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Derive `harmos.columns.<row>` (Every Row Field Repeated) and `harmos.batch.<row>`, Where
/// `<row>` Is the Row's Full Name With Dots Replaced by Underscores.
///
/// An interval row holds one value per field, so repeated, map and oneof fields
/// (including proto3 `optional`) are refused with the action to take.
pub fn derive(row: &MessageDescriptor) -> Result<Derived, String> {
    let mut fields = Vec::new();
    let mut dependency = vec![row.parent_file().name().to_owned()];
    for field in row.fields() {
        if field.is_list() || field.is_map() || field.containing_oneof().is_some() {
            return Err(format!(
                "Declare Interval Stream: field `{}` of row `{}` is repeated, a map or in a oneof; declare a row with one singular \
                 value per field",
                field.name(),
                row.full_name()
            ));
        }
        let (kind, type_name) = match field.kind() {
            Kind::Message(message) => {
                dependency.push(message.parent_file().name().to_owned());
                (Type::Message, Some(format!(".{}", message.full_name())))
            }
            Kind::Enum(value) => {
                dependency.push(value.parent_file().name().to_owned());
                (Type::Enum, Some(format!(".{}", value.full_name())))
            }
            Kind::Double => (Type::Double, None),
            Kind::Float => (Type::Float, None),
            Kind::Int64 => (Type::Int64, None),
            Kind::Uint64 => (Type::Uint64, None),
            Kind::Int32 => (Type::Int32, None),
            Kind::Fixed64 => (Type::Fixed64, None),
            Kind::Fixed32 => (Type::Fixed32, None),
            Kind::Bool => (Type::Bool, None),
            Kind::String => (Type::String, None),
            Kind::Bytes => (Type::Bytes, None),
            Kind::Uint32 => (Type::Uint32, None),
            Kind::Sfixed32 => (Type::Sfixed32, None),
            Kind::Sfixed64 => (Type::Sfixed64, None),
            Kind::Sint32 => (Type::Sint32, None),
            Kind::Sint64 => (Type::Sint64, None),
        };
        fields.push(FieldDescriptorProto {
            name: Some(field.name().to_owned()),
            number: Some(field.number() as i32),
            label: Some(Label::Repeated as i32),
            r#type: Some(kind as i32),
            type_name,
            json_name: Some(field.json_name().to_owned()),
            ..Default::default()
        });
    }
    let name = row.full_name().replace('.', "_");
    let scalar = |name: &str, number: u32, kind: Type| FieldDescriptorProto {
        name: Some(name.into()),
        number: Some(number as i32),
        label: Some(Label::Optional as i32),
        r#type: Some(kind as i32),
        ..Default::default()
    };
    dependency.sort();
    dependency.dedup();
    let columns_file = FileDescriptorProto {
        name: Some(format!("harmos/columns/{}.proto", row.full_name())),
        package: Some("harmos.columns".into()),
        dependency,
        message_type: vec![DescriptorProto {
            name: Some(name.clone()),
            field: fields,
            ..Default::default()
        }],
        syntax: Some("proto3".into()),
        ..Default::default()
    };
    let batch_file = FileDescriptorProto {
        name: Some(format!("harmos/batch/{}.proto", row.full_name())),
        package: Some("harmos.batch".into()),
        dependency: vec![columns_file.name().to_owned()],
        message_type: vec![DescriptorProto {
            name: Some(name.clone()),
            field: vec![
                scalar("start_unix_nanos", batch::START_UNIX_NANOS, Type::Int64),
                scalar("period_nanos", batch::PERIOD_NANOS, Type::Uint64),
                scalar("count", batch::COUNT, Type::Uint64),
                FieldDescriptorProto {
                    type_name: Some(format!(".harmos.columns.{name}")),
                    ..scalar("columns", batch::COLUMNS, Type::Message)
                },
            ],
            ..Default::default()
        }],
        syntax: Some("proto3".into()),
        ..Default::default()
    };
    let mut pool = row.parent_pool().clone();
    // A Second Derivation From the Same Pool Finds the Messages Already Added.
    for file in [columns_file, batch_file] {
        if pool.get_file_by_name(file.name()).is_none() {
            pool.add_file_descriptor_proto(file).map_err(|error| format!("Declare Interval Stream: {error}"))?;
        }
    }
    let columns = pool.get_message_by_name(&format!("harmos.columns.{name}")).expect("derived columns message");
    let batch = pool.get_message_by_name(&format!("harmos.batch.{name}")).expect("derived batch message");
    Ok(Derived { columns, batch })
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use prost_reflect::DescriptorPool;

    fn pool(fields: Vec<FieldDescriptorProto>) -> DescriptorPool {
        let file = FileDescriptorProto {
            name: Some("fixture/row.proto".into()),
            package: Some("fixture".into()),
            message_type: vec![DescriptorProto {
                name: Some("Row".into()),
                field: fields,
                ..Default::default()
            }],
            syntax: Some("proto3".into()),
            ..Default::default()
        };
        DescriptorPool::from_file_descriptor_set(prost_types::FileDescriptorSet { file: vec![file] }).unwrap()
    }

    fn field(name: &str, number: i32, kind: Type, label: Label) -> FieldDescriptorProto {
        FieldDescriptorProto {
            name: Some(name.into()),
            number: Some(number),
            label: Some(label as i32),
            r#type: Some(kind as i32),
            ..Default::default()
        }
    }

    #[test]
    fn columns_repeat_every_row_field_with_its_number() {
        let pool = pool(vec![field("value", 3, Type::Double, Label::Optional), field("name", 7, Type::String, Label::Optional)]);
        let derived = derive(&pool.get_message_by_name("fixture.Row").unwrap()).unwrap();
        let columns = derived.columns.fields().map(|field| (field.number(), field.is_list(), field.is_packed())).collect::<Vec<_>>();
        assert_eq!(columns, [(3, true, true), (7, true, false)]);
        let batch = derived.batch.fields().map(|field| field.name().to_owned()).collect::<Vec<_>>();
        assert_eq!(batch, ["start_unix_nanos", "period_nanos", "count", "columns"]);
    }

    #[test]
    fn repeated_row_fields_are_refused() {
        let pool = pool(vec![field("values", 1, Type::Double, Label::Repeated)]);
        let error = derive(&pool.get_message_by_name("fixture.Row").unwrap()).unwrap_err();
        assert!(error.contains("`values`"), "{error}");
    }
}
