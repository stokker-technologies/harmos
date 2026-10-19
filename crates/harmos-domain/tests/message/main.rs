/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod aggregate;
mod allocation;
mod boxed;
mod enums;
mod graph;
mod host;
mod native;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_domain::{MemorySize, Storable, message};
use harmos_sdk::grpc::{self, DecodeInput, prost};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
struct Original {
    value: u64,
    payload: Vec<u8>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
struct Reordered {
    payload: Vec<u8>,
    value: u64,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
struct Added {
    extra: String,
    value: u64,
    payload: Vec<u8>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
struct Renamed {
    renamed: u64,
    payload: Vec<u8>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq, Default)]
struct Nested {
    row: Original,
    optional: Option<Original>,
    count: Option<u32>,
    rows: Vec<Original>,
    readings: Vec<f64>,
    signed: i64,
    number: f32,
    enabled: bool,
    r#type: String,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
struct QualifiedBytes {
    bytes: Vec<std::primitive::u8>,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct DecoderNames {
    input: u64,
    field: String,
    owned: Vec<u8>,
    layout: Option<Original>,
    value: Vec<u32>,
    __harmos_input: bool,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
struct Choices {
    selected: Choice,
    optional: Option<Choice>,
    history: Vec<Choice>,
}

/* === Enums === */

#[message(harmos_path = ::harmos_domain)]
#[derive(Debug, PartialEq)]
enum Choice {
    Empty,
    Text(String),
    Sample { enabled: bool, row: Original, bytes: Vec<u8>, values: Vec<i64>, note: Option<String> },
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Debug, PartialEq)]
enum ReorderedChoice {
    Sample { note: Option<String>, values: Vec<i64>, bytes: Vec<u8>, row: Original, enabled: bool },
    Text(String),
    Empty,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Debug, Default, PartialEq)]
enum ExplicitDefault {
    Other(String),

    #[default]
    Selected,
}

#[message(harmos_path = ::harmos_domain)]
#[derive(Debug, PartialEq)]
enum MergedChoice {
    Empty,
    Pair { left: String, right: String },
}

#[message(harmos_path = ::harmos_domain)]
#[derive(PartialEq)]
enum SingleVariant {
    Named { r#type: String },
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

// Encode A Fixture Through The Shared Transport Adapter.
fn encode<T: harmos_domain::Message + harmos_domain::schema::Described>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    grpc::encode(value, &mut bytes).unwrap();
    bytes
}

fn protobuf_fields(definition: harmos_domain::schema::declaration::Definition) -> harmos_domain::schema::graph::Schema {
    let mut schema =
        harmos_domain::schema::graph::Schema::from_declaration(definition, &harmos_domain::config::SchemaSettings::default()).unwrap();
    for fields in &mut schema.definitions {
        fields.sort_by_key(|field| field.tag);
    }
    schema
}

fn schema_descriptor(definition: harmos_domain::schema::declaration::Definition) -> harmos_packaging::proto::reflection::MessageDescriptor {
    let settings = harmos_domain::config::SchemaSettings::default();
    let graph = harmos_domain::schema::graph::Schema::from_declaration(definition, &settings).unwrap();
    harmos_packaging::proto::reflection::descriptor_schema(&graph, "harmos.stream", &settings).unwrap()
}

/* === Tests === */

#[test]
fn names_never_shadow() {
    let value = DecoderNames {
        input: 7,
        field: "named".into(),
        owned: vec![255],
        layout: Some(Original { value: 9, payload: vec![1] }),
        value: vec![1, 128],
        __harmos_input: true,
    };
    let encoded = crate::encode(&value);
    let budget = grpc::decoding_memory::<DecoderNames>(DecodeInput::new(&encoded)).unwrap();
    let decoded = grpc::decode::<DecoderNames>(DecodeInput::new(&encoded)).unwrap();
    assert_eq!(decoded, value);
    assert!(decoded.memory_size() <= budget);
}

#[test]
fn names_survive_reordering() {
    let original = Original { value: u64::MAX, payload: vec![0, 128, 255] };
    let bytes = crate::encode(&original);
    let reordered = grpc::decode::<Reordered>(DecodeInput::new(&bytes)).unwrap();
    assert_eq!(reordered.value, original.value);
    assert_eq!(reordered.payload, original.payload);
    assert_eq!(crate::encode(&reordered), bytes);
    let added = grpc::decode::<Added>(DecodeInput::new(&bytes)).unwrap();
    assert!(added.extra.is_empty());
    assert_eq!(grpc::decode::<Original>(DecodeInput::new(&crate::encode(&Added { extra: "ignored".into(), ..added }))).unwrap(), original);
    let renamed = grpc::decode::<Renamed>(DecodeInput::new(&bytes)).unwrap();
    assert_eq!(renamed.renamed, 0, "a rename loses the old wire identity");
    assert_eq!(renamed.payload, original.payload);
}

#[test]
fn nested_values_defaults() {
    let empty = grpc::decode::<Nested>(DecodeInput::new(&[])).unwrap();
    assert_eq!(empty.row.value, 0);
    assert_eq!(empty.optional, None);
    assert_eq!(empty.count, None);
    assert!(empty.rows.is_empty());
    let value = Nested {
        row: Original { value: u64::MAX, payload: vec![0, 255] },
        optional: Some(Original::default()),
        count: Some(0),
        rows: vec![Original::default()],
        readings: vec![-1.0, f64::INFINITY],
        signed: i64::MIN,
        number: f32::NEG_INFINITY,
        enabled: true,
        r#type: "probe".into(),
    };
    let bytes = crate::encode(&value);
    assert_eq!(grpc::decode::<Nested>(DecodeInput::new(&bytes)).unwrap(), value);
    assert!(grpc::decode::<Nested>(DecodeInput::new(&[0xff])).is_err());
    assert_eq!(grpc::decode::<String>(DecodeInput::new(&crate::encode(&"hello".to_owned()))).unwrap(), "hello");
    grpc::decode::<()>(DecodeInput::new(&[])).unwrap();
}

#[test]
fn bytes_packed_numbers() {
    #[message(harmos_path = ::harmos_domain)]
    struct Bytes {
        payload: Vec<u8>,
    }
    #[message(harmos_path = ::harmos_domain)]
    struct Numbers {
        payload: Vec<u32>,
    }
    let bytes = crate::encode(&Bytes { payload: vec![1, 2, 127] });
    assert_eq!(bytes, crate::encode(&Numbers { payload: vec![1, 2, 127] }));
    assert_eq!(bytes.len(), 9, "five-byte tag, one-byte length, three raw bytes");
    assert_eq!(&bytes[5..], &[3, 1, 2, 127]);
}

#[test]
fn schemas_preserve_wire() {
    use harmos_domain::schema::{Cardinality, Described, Kind};
    assert_ne!(Original::FIELDS, Reordered::FIELDS);
    let left = protobuf_fields(Original::DEFINITION);
    let right = protobuf_fields(Reordered::DEFINITION);
    assert!(left.equivalent(left.root, &right, right.root, &harmos_domain::config::SchemaSettings::default()).unwrap());
    let value = Original::FIELDS.iter().find(|field| field.name == "value").unwrap();
    assert_eq!(value.tag, 39_772_037);
    assert_eq!(value.kind, Kind::U64);
    let payload = Original::FIELDS.iter().find(|field| field.name == "payload").unwrap();
    assert_eq!(payload.kind, Kind::Bytes);
    for (name, cardinality) in [("row", Cardinality::Required), ("optional", Cardinality::Optional), ("rows", Cardinality::Repeated)] {
        let field = Nested::FIELDS.iter().find(|field| field.name == name).unwrap();
        assert_eq!(field.cardinality, cardinality);
        let Kind::Message { definition } = &field.kind else { panic!("nested message descriptor") };
        assert_eq!((definition.identity)(), std::any::TypeId::of::<Original>());
        assert_eq!((definition.fields)(), Original::FIELDS);
    }
    let graph =
        harmos_domain::schema::graph::Schema::from_declaration(Nested::DEFINITION, &harmos_domain::config::SchemaSettings::default())
            .unwrap();
    let encoded = serde_json::to_vec(&graph).unwrap();
    let owned: harmos_domain::schema::graph::Schema = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(owned, graph);
}

#[test]
fn fields_keep_binary() {
    use harmos_domain::schema::{Described, Kind};
    assert_eq!(QualifiedBytes::FIELDS[0].kind, Kind::Bytes);
    let value = QualifiedBytes { bytes: vec![0, 255] };
    assert_eq!(grpc::decode::<QualifiedBytes>(DecodeInput::new(&crate::encode(&value))).unwrap(), value);
}

#[test]
fn sizes_count_capacity() {
    let mut payload = Vec::with_capacity(64);
    payload.extend_from_slice(&[1, 2, 3]);
    let row = Original { value: 7, payload };
    let row_heap = row.payload.capacity();
    let mut rows = Vec::with_capacity(4);
    rows.push(row.clone());
    let rows_heap = rows.capacity() * std::mem::size_of::<Original>() + rows[0].payload.capacity();
    let mut name = String::with_capacity(80);
    name.push_str("sample");
    let mut value =
        Nested { row, optional: Some(Original { value: 9, payload: Vec::with_capacity(32) }), rows, r#type: name, ..Default::default() };
    let expected = std::mem::size_of::<Nested>() + row_heap + 32 + rows_heap + 80;
    assert_eq!(value.memory_size(), expected);
    assert_eq!(value.encoded_size(), harmos_domain::Message::encode_to_vec(&value).len());

    value.optional = None;
    assert_eq!(value.memory_size(), expected - 32);
    assert_eq!(value.encoded_size(), harmos_domain::Message::encode_to_vec(&value).len());
    assert!(value.memory_size() > value.encoded_size());
}

#[test]
fn allocation_never_small() {
    struct Overflow;
    impl MemorySize for Overflow {
        fn memory_size(&self) -> usize {
            usize::MAX
        }
    }
    assert_eq!(Some(Overflow).memory_size(), usize::MAX);
    assert_eq!(vec![Overflow].memory_size(), usize::MAX);
    assert_eq!(None::<Overflow>.memory_size(), std::mem::size_of::<Option<Overflow>>());
}

#[test]
fn enum_shapes_sizes() {
    use harmos_domain::schema::{Cardinality, Described};
    let choices = [
        Choice::Empty,
        Choice::Text("value".to_owned()),
        Choice::Sample {
            enabled: false,
            row: Original { value: 42, payload: vec![0, 255] },
            bytes: vec![1, 2],
            values: vec![i64::MIN, 0, 3],
            note: Some(String::new()),
        },
    ];
    for value in choices {
        let encoded = crate::encode(&value);
        assert_eq!(grpc::decode::<Choice>(DecodeInput::new(&encoded)).unwrap(), value);
        let reordered = grpc::decode::<ReorderedChoice>(DecodeInput::new(&encoded)).unwrap();
        assert_eq!(grpc::decode::<Choice>(DecodeInput::new(&crate::encode(&reordered))).unwrap(), value);
    }
    let nested = Choices {
        selected: Choice::Text(String::new()),
        optional: Some(Choice::Empty),
        history: vec![Choice::Empty, Choice::Text("owned".into())],
    };
    assert_eq!(grpc::decode::<Choices>(DecodeInput::new(&crate::encode(&nested))).unwrap(), nested);
    assert_ne!(Choice::FIELDS, ReorderedChoice::FIELDS);
    let left = protobuf_fields(Choice::DEFINITION);
    let right = protobuf_fields(ReorderedChoice::DEFINITION);
    assert!(left.equivalent(left.root, &right, right.root, &harmos_domain::config::SchemaSettings::default()).unwrap());
    assert!(Choice::FIELDS.iter().all(|field| field.cardinality == Cardinality::Variant));
    harmos_domain::schema::graph::Schema::from_declaration(Choice::DEFINITION, &harmos_domain::config::SchemaSettings::default()).unwrap();
    assert_eq!(grpc::decode::<Choice>(DecodeInput::new(&[])).unwrap(), Choice::Empty);
    assert_eq!(grpc::decode::<ExplicitDefault>(DecodeInput::new(&[])).unwrap(), ExplicitDefault::Selected);
    assert!(grpc::decode::<Choice>(DecodeInput::new(&[0xff])).is_err());
    let mut text = String::with_capacity(77);
    text.push_str("resident");
    let value = Choice::Text(text);
    assert_eq!(value.memory_size(), std::mem::size_of::<Choice>() + 77);
    assert_eq!(value.encoded_size(), harmos_domain::Message::encode_to_vec(&value).len());
}

#[test]
fn wire_uses_last() {
    let mut bytes = crate::encode(&Choice::Text("before".into()));
    bytes.extend(crate::encode(&Choice::Empty));
    bytes.extend([8, 1]);
    assert_eq!(grpc::decode::<Choice>(DecodeInput::new(&bytes)).unwrap(), Choice::Empty);
}

#[test]
fn variant_keeps_raw() {
    use harmos_domain::schema::{Described, Kind};
    let variant = Choice::FIELDS.iter().find(|field| field.name == "Text").unwrap();
    let Kind::Message { definition } = &variant.kind else {
        panic!("variant payload");
    };
    let fields = (definition.fields)();
    let mut wrong_type = Vec::new();
    prost::encoding::uint64::encode(fields[0].tag, &7, &mut wrong_type);
    let mut bytes = Vec::new();
    prost::encoding::bytes::encode(variant.tag, &wrong_type, &mut bytes);
    assert!(grpc::decode::<Choice>(DecodeInput::new(&bytes)).is_err());
    assert!(grpc::decoding_memory::<Choice>(DecodeInput::new(&bytes)).is_err());
    bytes.extend(crate::encode(&Choice::Empty));
    assert_eq!(grpc::decode::<Choice>(DecodeInput::new(&bytes)).unwrap(), Choice::Empty);
    assert!(grpc::decoding_memory::<Choice>(DecodeInput::new(&bytes)).unwrap() >= std::mem::size_of::<Choice>());

    let mut malformed = Vec::new();
    prost::encoding::bytes::encode(variant.tag, &vec![255], &mut malformed);
    malformed.extend(crate::encode(&Choice::Empty));
    assert!(grpc::decode::<Choice>(DecodeInput::new(&malformed)).is_err());
    assert!(grpc::decoding_memory::<Choice>(DecodeInput::new(&malformed)).is_err());
}

#[test]
fn declared_unit_default() {
    let budget = grpc::decoding_memory::<ExplicitDefault>(DecodeInput::new(&[])).unwrap();
    let value = grpc::decode::<ExplicitDefault>(DecodeInput::new(&[])).unwrap();
    assert_eq!(value, ExplicitDefault::Selected);
    // The Transcoding Budget Bounds The Decoded Value From Above.
    assert!(budget >= std::mem::size_of::<ExplicitDefault>());
    assert!(budget >= value.memory_size());
}

#[test]
fn enums_use_oneof() {
    use harmos_domain::schema::Described;
    use harmos_packaging::proto::reflection::DynamicMessage;
    let schema = schema_descriptor(Choice::DEFINITION);
    assert_eq!(schema.oneofs().count(), 1);
    let value = Choice::Text("reflected".into());
    let reflected = DynamicMessage::decode(schema, crate::encode(&value).as_slice()).unwrap();
    let bytes = prost::Message::encode_to_vec(&reflected);
    assert_eq!(grpc::decode::<Choice>(DecodeInput::new(&bytes)).unwrap(), value);
}

#[test]
fn composites_match_canonical() {
    use harmos_domain::schema::Described;
    use harmos_packaging::proto::reflection::DynamicMessage;
    fn canonical<T: harmos_domain::Message + Described>(value: &T) {
        let encoded = crate::encode(value);
        let reflected = DynamicMessage::decode(schema_descriptor(T::DEFINITION), encoded.as_slice()).unwrap();
        assert_eq!(encoded, prost::Message::encode_to_vec(&reflected));
    }
    canonical(&Nested {
        row: Original { value: 7, payload: vec![255] },
        optional: Some(Original { value: 9, payload: vec![1, 2] }),
        count: Some(0),
        rows: vec![Original { value: 13, payload: vec![3] }],
        signed: 17,
        r#type: "typed".into(),
        ..Default::default()
    });
    for (left, right) in [("left", "right"), ("", "right"), ("left", ""), ("", "")] {
        canonical(&MergedChoice::Pair { left: left.into(), right: right.into() });
    }
}

#[test]
fn variant_merge_payload() {
    use harmos_domain::schema::{Described, Kind};
    let variant = MergedChoice::FIELDS.iter().find(|field| field.name == "Pair").unwrap();
    let Kind::Message { definition } = &variant.kind else { unreachable!() };
    let fields = (definition.fields)();
    let mut wire = Vec::new();
    for (name, text) in [("left", "first"), ("right", "second")] {
        let field = fields.iter().find(|field| field.name == name).unwrap();
        let mut payload = Vec::new();
        prost::encoding::string::encode(field.tag, &text.to_owned(), &mut payload);
        prost::encoding::bytes::encode(variant.tag, &payload, &mut wire);
    }
    assert_eq!(
        grpc::decode::<MergedChoice>(DecodeInput::new(&wire)).unwrap(),
        MergedChoice::Pair { left: "first".into(), right: "second".into() }
    );
}

#[test]
fn variants_preserve_raw() {
    let value = SingleVariant::Named { r#type: "raw".into() };
    assert_eq!(grpc::decode::<SingleVariant>(DecodeInput::new(&crate::encode(&value))).unwrap(), value);
    assert_eq!(grpc::decode::<SingleVariant>(DecodeInput::new(&[])).unwrap(), SingleVariant::Named { r#type: String::new() });
}

#[test]
fn validation_accepts_generated() {
    use grpc::Validator;
    use harmos_domain::schema::Described;
    let value = Choices { selected: Choice::Text("checked".into()), optional: Some(Choice::Empty), history: vec![Choice::Empty] };
    let validator = Validator::from_schema(
        &harmos_domain::schema::graph::Schema::from_declaration(Choices::DEFINITION, &harmos_domain::config::SchemaSettings::default())
            .unwrap(),
        &harmos_domain::config::SchemaSettings::default(),
    )
    .unwrap();
    validator.validate(&crate::encode(&value)).unwrap();
    let validator = Validator::from_schema(
        &harmos_domain::schema::graph::Schema::from_declaration(Choice::DEFINITION, &harmos_domain::config::SchemaSettings::default())
            .unwrap(),
        &harmos_domain::config::SchemaSettings::default(),
    )
    .unwrap();
    validator.validate(&crate::encode(&Choice::Text("checked".into()))).unwrap();
}
