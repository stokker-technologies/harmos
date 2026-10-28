/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Inert Protocol Generation Fixtures

use harmos_domain::operations::{OperationPolicy, RetryPolicy};

const POLICY: OperationPolicy = OperationPolicy {
    timeout: std::time::Duration::from_secs(1),
    attempt_timeout: std::time::Duration::from_secs(1),
    retries: RetryPolicy::Never,
};

#[harmos_domain::message(harmos_path = ::harmos_domain)]
pub(crate) struct Payload {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::operation(
    id = "read",
    harmos_path = ::harmos_domain,
    response = Payload,
    policy = POLICY,
    grpc(package = "test.v1", service = "Shapes", method = "Read"),
)]
pub(crate) struct ReadOperation {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::operation(
    id = "exchange",
    harmos_path = ::harmos_domain,
    response = Payload,
    input = Payload,
    output = Payload,
    policy = POLICY,
    grpc(package = "test.v1", service = "Shapes", method = "Exchange"),
)]
pub(crate) struct ExchangeOperation {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::operation(
    id = "upload",
    harmos_path = ::harmos_domain,
    response = Payload,
    input = Payload,
    policy = POLICY,
    grpc(package = "test.v1", service = "Shapes", method = "Upload"),
)]
pub(crate) struct UploadOperation {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::operation(
    id = "download",
    harmos_path = ::harmos_domain,
    response = Payload,
    output = Payload,
    policy = POLICY,
    grpc(package = "test.v1", service = "Shapes", method = "Download"),
)]
pub(crate) struct DownloadOperation {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::operation(
    harmos_path = ::harmos_domain,
    id = "annotate",
    response = AnnotatedOperation,
    policy = POLICY,
    grpc(package = "test.v1", service = "Portable", method = "Annotate"),
)]
#[derive(Clone, PartialEq)]
pub(crate) struct AnnotatedOperation {
    identifier: harmos_domain::Identifier,
    metadata: harmos_domain::Metadata,
    value: harmos_domain::Value,
    optional: Option<harmos_domain::Value>,
    values: Vec<harmos_domain::Value>,
}

#[harmos_domain::operation(
    harmos_path = ::harmos_domain,
    id = "wrap",
    response = Delivered,
    policy = POLICY,
    grpc(package = "test.v1", service = "Boxed", method = "Wrap"),
)]
#[derive(Clone, PartialEq)]
pub(crate) struct EnvelopeOperation {
    pub(crate) inner: Box<AnnotatedOperation>,
    pub(crate) sequence: u64,
}

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
    Boxed(Box<AnnotatedOperation>),
}

#[harmos_domain::catalog(harmos_path = ::harmos_domain)]
pub(crate) enum Boxed {
    Wrap(EnvelopeOperation),
}

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
