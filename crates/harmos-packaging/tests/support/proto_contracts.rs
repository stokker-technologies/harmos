/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Inert Protocol Generation Fixtures

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

use harmos_domain::operations::{OperationPolicy, RetryPolicy};

/* === Constants === */

const POLICY: OperationPolicy = OperationPolicy {
    timeout: std::time::Duration::from_secs(1),
    attempt_timeout: std::time::Duration::from_secs(1),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_domain::message(harmos_path = ::harmos_domain)]
pub(crate) struct Payload {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[::harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(::harmos_domain::Request)]
#[harmos_path(::harmos_domain)]
#[request(response = Payload)]
#[derive(Clone)]
pub(crate) struct ReadOperation {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

pub(crate) static READ_OPERATION_DESCRIPTION: ::harmos_domain::operations::WireDeclaration =
    ::harmos_domain::operations::WireDeclaration::request::<ReadOperation>("read", POLICY)
        .visibility(::harmos_domain::operations::Visibility::Public)
        .grpc(::harmos_domain::operations::Grpc::new("test.v1", "Shapes", "Read"));

pub(crate) const READ_OPERATION_OPERATION: ::harmos_domain::operations::OperationSpec<ReadOperation> =
    ::harmos_domain::operations::OperationSpec::<ReadOperation>::new(&READ_OPERATION_DESCRIPTION);

#[::harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(::harmos_domain::Request)]
#[harmos_path(::harmos_domain)]
#[request(response = Payload, input = Payload, output = Payload)]
#[derive(Clone)]
pub(crate) struct ExchangeOperation {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

pub(crate) static EXCHANGE_OPERATION_DESCRIPTION: ::harmos_domain::operations::WireDeclaration =
    ::harmos_domain::operations::WireDeclaration::request::<ExchangeOperation>("exchange", POLICY)
        .visibility(::harmos_domain::operations::Visibility::Public)
        .grpc(::harmos_domain::operations::Grpc::new("test.v1", "Shapes", "Exchange"));

pub(crate) const EXCHANGE_OPERATION_OPERATION: ::harmos_domain::operations::OperationSpec<ExchangeOperation> =
    ::harmos_domain::operations::OperationSpec::<ExchangeOperation>::new(&EXCHANGE_OPERATION_DESCRIPTION);

#[::harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(::harmos_domain::Request)]
#[harmos_path(::harmos_domain)]
#[request(response = Payload, input = Payload)]
#[derive(Clone)]
pub(crate) struct UploadOperation {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

pub(crate) static UPLOAD_OPERATION_DESCRIPTION: ::harmos_domain::operations::WireDeclaration =
    ::harmos_domain::operations::WireDeclaration::request::<UploadOperation>("upload", POLICY)
        .visibility(::harmos_domain::operations::Visibility::Public)
        .grpc(::harmos_domain::operations::Grpc::new("test.v1", "Shapes", "Upload"));

pub(crate) const UPLOAD_OPERATION_OPERATION: ::harmos_domain::operations::OperationSpec<UploadOperation> =
    ::harmos_domain::operations::OperationSpec::<UploadOperation>::new(&UPLOAD_OPERATION_DESCRIPTION);

#[::harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(::harmos_domain::Request)]
#[harmos_path(::harmos_domain)]
#[request(response = Payload, output = Payload)]
#[derive(Clone)]
pub(crate) struct DownloadOperation {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

pub(crate) static DOWNLOAD_OPERATION_DESCRIPTION: ::harmos_domain::operations::WireDeclaration =
    ::harmos_domain::operations::WireDeclaration::request::<DownloadOperation>("download", POLICY)
        .visibility(::harmos_domain::operations::Visibility::Public)
        .grpc(::harmos_domain::operations::Grpc::new("test.v1", "Shapes", "Download"));

pub(crate) const DOWNLOAD_OPERATION_OPERATION: ::harmos_domain::operations::OperationSpec<DownloadOperation> =
    ::harmos_domain::operations::OperationSpec::<DownloadOperation>::new(&DOWNLOAD_OPERATION_DESCRIPTION);

#[::harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(::harmos_domain::Request)]
#[harmos_path(::harmos_domain)]
#[request(response = AnnotatedOperation)]
#[derive(Clone, PartialEq)]
pub(crate) struct AnnotatedOperation {
    identifier: harmos_domain::Identifier,
    metadata: harmos_domain::Metadata,
    value: harmos_domain::Value,
    optional: Option<harmos_domain::Value>,
    values: Vec<harmos_domain::Value>,
}

pub(crate) static ANNOTATED_OPERATION_DESCRIPTION: ::harmos_domain::operations::WireDeclaration =
    ::harmos_domain::operations::WireDeclaration::request::<AnnotatedOperation>("annotate", POLICY)
        .visibility(::harmos_domain::operations::Visibility::Public)
        .grpc(::harmos_domain::operations::Grpc::new("test.v1", "Portable", "Annotate"));

pub(crate) const ANNOTATED_OPERATION_OPERATION: ::harmos_domain::operations::OperationSpec<AnnotatedOperation> =
    ::harmos_domain::operations::OperationSpec::<AnnotatedOperation>::new(&ANNOTATED_OPERATION_DESCRIPTION);

#[::harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(::harmos_domain::Request)]
#[harmos_path(::harmos_domain)]
#[request(response = Delivered)]
#[derive(Clone, PartialEq)]
pub(crate) struct EnvelopeOperation {
    pub(crate) inner: Box<AnnotatedOperation>,
    pub(crate) sequence: u64,
}

pub(crate) static ENVELOPE_OPERATION_DESCRIPTION: ::harmos_domain::operations::WireDeclaration =
    ::harmos_domain::operations::WireDeclaration::request::<EnvelopeOperation>("wrap", POLICY)
        .visibility(::harmos_domain::operations::Visibility::Public)
        .grpc(::harmos_domain::operations::Grpc::new("test.v1", "Boxed", "Wrap"));

pub(crate) const ENVELOPE_OPERATION_OPERATION: ::harmos_domain::operations::OperationSpec<EnvelopeOperation> =
    ::harmos_domain::operations::OperationSpec::<EnvelopeOperation>::new(&ENVELOPE_OPERATION_DESCRIPTION);

/* === Enums === */

static ROUTES_CATALOG_DECLARATIONS: [harmos_domain::catalog::Node<harmos_domain::operations::WireDeclaration>; 4] = [
    harmos_domain::catalog::Node::Leaf(READ_OPERATION_OPERATION.wire()),
    harmos_domain::catalog::Node::Leaf(EXCHANGE_OPERATION_OPERATION.wire()),
    harmos_domain::catalog::Node::Leaf(UPLOAD_OPERATION_OPERATION.wire()),
    harmos_domain::catalog::Node::Leaf(DOWNLOAD_OPERATION_OPERATION.wire()),
];
static ROUTES_CATALOG_CALLEES: [harmos_domain::catalog::Node<&'static [harmos_domain::operations::WireDeclaration]>; 4] = [
    harmos_domain::catalog::Node::Leaf(READ_OPERATION_OPERATION.callees()),
    harmos_domain::catalog::Node::Leaf(EXCHANGE_OPERATION_OPERATION.callees()),
    harmos_domain::catalog::Node::Leaf(UPLOAD_OPERATION_OPERATION.callees()),
    harmos_domain::catalog::Node::Leaf(DOWNLOAD_OPERATION_OPERATION.callees()),
];

pub(crate) const ROUTES_CATALOG: harmos_operations::execution::operations::Catalog = harmos_operations::execution::operations::Catalog::new(
    concat!(module_path!(), "::Routes"),
    &ROUTES_CATALOG_DECLARATIONS,
    &ROUTES_CATALOG_CALLEES,
    None,
);

static PORTABLE_CATALOG_DECLARATIONS: [harmos_domain::catalog::Node<harmos_domain::operations::WireDeclaration>; 1] =
    [harmos_domain::catalog::Node::Leaf(ANNOTATED_OPERATION_OPERATION.wire())];
static PORTABLE_CATALOG_CALLEES: [harmos_domain::catalog::Node<&'static [harmos_domain::operations::WireDeclaration]>; 1] =
    [harmos_domain::catalog::Node::Leaf(ANNOTATED_OPERATION_OPERATION.callees())];

pub(crate) const PORTABLE_CATALOG: harmos_operations::execution::operations::Catalog =
    harmos_operations::execution::operations::Catalog::new(
        concat!(module_path!(), "::Portable"),
        &PORTABLE_CATALOG_DECLARATIONS,
        &PORTABLE_CATALOG_CALLEES,
        None,
    );

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
pub(crate) enum Delivered {
    Missing,
    Boxed(Box<AnnotatedOperation>),
}

static BOXED_CATALOG_DECLARATIONS: [harmos_domain::catalog::Node<harmos_domain::operations::WireDeclaration>; 1] =
    [harmos_domain::catalog::Node::Leaf(ENVELOPE_OPERATION_OPERATION.wire())];
static BOXED_CATALOG_CALLEES: [harmos_domain::catalog::Node<&'static [harmos_domain::operations::WireDeclaration]>; 1] =
    [harmos_domain::catalog::Node::Leaf(ENVELOPE_OPERATION_OPERATION.callees())];

pub(crate) const BOXED_CATALOG: harmos_operations::execution::operations::Catalog = harmos_operations::execution::operations::Catalog::new(
    concat!(module_path!(), "::Boxed"),
    &BOXED_CATALOG_DECLARATIONS,
    &BOXED_CATALOG_CALLEES,
    None,
);

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn annotated() -> AnnotatedOperation {
    use harmos_domain::{Identifier, Metadata, Value};
    let mut metadata = Metadata::definition(Identifier::new("device.reading").unwrap(), "Reading", None, ["lab"]);
    metadata.attributes.insert("channel".into(), Value::Unsigned(1));
    AnnotatedOperation {
        identifier: Identifier::new("device").unwrap(),
        metadata,
        value: Value::Array(vec![Value::Text("x".into()), Value::Null]),
        optional: Some(Value::Number(2.5)),
        values: vec![Value::Bool(true), Value::Integer(-1)],
    }
}

/* === Private Functions === */

/* === Tests === */
