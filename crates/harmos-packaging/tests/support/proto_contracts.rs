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

use harmos_domain::operations::{OperationPolicy, RestartPolicy, RetryPolicy};

/* === Constants === */

const POLICY: OperationPolicy = OperationPolicy {
    timeout: std::time::Duration::from_secs(1),
    attempt_timeout: std::time::Duration::from_secs(1),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
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
#[harmos_domain::operation(harmos_path = ::harmos_domain, response = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Read"))]
pub(crate) struct Read {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[harmos_domain::operation(harmos_path = ::harmos_domain, response = Payload, input = Payload, output = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Exchange"))]
pub(crate) struct Exchange {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[harmos_domain::operation(harmos_path = ::harmos_domain, response = Payload, input = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Upload"))]
pub(crate) struct Upload {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[harmos_domain::operation(harmos_path = ::harmos_domain, response = Payload, output = Payload, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Shapes", method = "Download"))]
pub(crate) struct Download {
    text: String,
    optional: Option<u32>,
    values: Vec<i64>,
}

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[harmos_domain::operation(harmos_path = ::harmos_domain, id = "annotate", response = Annotated, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Portable", method = "Annotate"))]
#[derive(Clone, PartialEq)]
pub(crate) struct Annotated {
    identifier: harmos_domain::Identifier,
    metadata: harmos_domain::Metadata,
    value: harmos_domain::Value,
    optional: Option<harmos_domain::Value>,
    values: Vec<harmos_domain::Value>,
}

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[harmos_domain::operation(harmos_path = ::harmos_domain, id = "wrap", response = Delivered, policy = POLICY, version = 1, grpc(package = "test.v1", service = "Boxed", method = "Wrap"))]
#[derive(Clone, PartialEq)]
pub(crate) struct Envelope {
    pub(crate) inner: Box<Annotated>,
    pub(crate) sequence: u64,
}

/* === Enums === */

#[harmos_domain::catalog(harmos_path = ::harmos_domain)]
pub(crate) enum Routes {
    Read(Read),
    Exchange(Exchange),
    Upload(Upload),
    Download(Download),
}

#[harmos_domain::catalog(harmos_path = ::harmos_domain)]
pub(crate) enum Portable {
    Annotate(Annotated),
}

#[harmos_domain::message(harmos_path = ::harmos_domain)]
#[derive(Clone, PartialEq)]
pub(crate) enum Delivered {
    Missing,
    Boxed(Box<Annotated>),
}

#[harmos_domain::catalog(harmos_path = ::harmos_domain)]
pub(crate) enum Boxed {
    Wrap(Envelope),
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
