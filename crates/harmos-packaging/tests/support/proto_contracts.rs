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

#[harmos_domain::message(harmos_path = ::harmos_domain)]
pub(crate) struct Read {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::operation(request = Read, infer_request, id = "read", harmos_path = ::harmos_domain, response = Payload, policy = POLICY, grpc(package = "test.v1", service = "Shapes", method = "Read"))]
pub(crate) struct ReadOperation;

#[harmos_domain::message(harmos_path = ::harmos_domain)]
pub(crate) struct Exchange {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::operation(request = Exchange, infer_request, id = "exchange", harmos_path = ::harmos_domain, response = Payload, input = Payload, output = Payload, policy = POLICY, grpc(package = "test.v1", service = "Shapes", method = "Exchange"))]
pub(crate) struct ExchangeOperation;

#[harmos_domain::message(harmos_path = ::harmos_domain)]
pub(crate) struct Upload {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::operation(request = Upload, infer_request, id = "upload", harmos_path = ::harmos_domain, response = Payload, input = Payload, policy = POLICY, grpc(package = "test.v1", service = "Shapes", method = "Upload"))]
pub(crate) struct UploadOperation;

#[harmos_domain::message(harmos_path = ::harmos_domain)]
pub(crate) struct Download {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::operation(request = Download, infer_request, id = "download", harmos_path = ::harmos_domain, response = Payload, output = Payload, policy = POLICY, grpc(package = "test.v1", service = "Shapes", method = "Download"))]
pub(crate) struct DownloadOperation;

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
pub(crate) struct Annotated {
    identifier: harmos_domain::Identifier,
    metadata: harmos_domain::Metadata,
    value: harmos_domain::Value,
    optional: Option<harmos_domain::Value>,
    values: Vec<harmos_domain::Value>,
}

#[harmos_domain::operation(request = Annotated, infer_request, harmos_path = ::harmos_domain, id = "annotate", response = Annotated, policy = POLICY, grpc(package = "test.v1", service = "Portable", method = "Annotate"))]
pub(crate) struct AnnotatedOperation;

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
pub(crate) struct Envelope {
    pub(crate) inner: Box<Annotated>,
    pub(crate) sequence: u64,
}

#[harmos_domain::operation(request = Envelope, infer_request, harmos_path = ::harmos_domain, id = "wrap", response = Delivered, policy = POLICY, grpc(package = "test.v1", service = "Boxed", method = "Wrap"))]
pub(crate) struct EnvelopeOperation;

/* === Enums === */

#[harmos_domain::catalog(harmos_path = ::harmos_domain)]
pub(crate) enum Routes {
    Read(ReadOperation),
    Exchange(ExchangeOperation),
    Upload(UploadOperation),
    Download(DownloadOperation),
}

#[harmos_domain::catalog(harmos_path = ::harmos_domain)]
pub(crate) enum Portable {
    Annotate(AnnotatedOperation),
}

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
pub(crate) enum Delivered {
    Missing,
    Boxed(Box<Annotated>),
}

#[harmos_domain::catalog(harmos_path = ::harmos_domain)]
pub(crate) enum Boxed {
    Wrap(EnvelopeOperation),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn annotated() -> Annotated {
    use harmos_domain::{Identifier, Metadata, Value};
    let mut metadata = Metadata::definition(Identifier::new("device.reading").unwrap(), "Reading", None, ["lab"]);
    metadata.attributes.insert("channel".into(), Value::Unsigned(1));
    Annotated {
        identifier: Identifier::new("device").unwrap(),
        metadata,
        value: Value::Array(vec![Value::Text("x".into()), Value::Null]),
        optional: Some(Value::Number(2.5)),
        values: vec![Value::Bool(true), Value::Integer(-1)],
    }
}

/* === Private Functions === */

/* === Tests === */
