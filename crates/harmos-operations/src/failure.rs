/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// One operation failure as every carrier transports it. The cause is written
// once where the error originates, the operation frame is added once where a
// call crosses from a plugin into the host, and the step frame belongs to the
// consumer. Sidecar frames, host refusals, gRPC status details and guest
// records carry the same value; no carrier re-embeds another's text.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::backtrace::{Backtrace, BacktraceStatus};
use std::sync::{Arc, OnceLock};
use std::{error, fmt, io, iter};

// Third Party Imports
use prost::Message;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

// Local Imports
use crate::operations::FailureKind;
use harmos_protocol::model::{self, Classification};

/* === Constants === */

const LINK_LIMIT: usize = 4 * 1024;

const CHAIN_LIMIT: usize = 16;

const BACKTRACE_LIMIT: usize = 32 * 1024;

const VERSION: u8 = 1;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// Boxed so every error enum that carries a failure stays one pointer wide.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Failure(Box<Record>);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    #[serde(serialize_with = "serialize_carrier", deserialize_with = "deserialize_carrier")]
    carrier: model::Failure,

    // Immutable views preserve the borrowed accessors; the carrier owns all fields.
    #[serde(skip)]
    context: OnceLock<Context>,

    #[serde(skip)]
    diagnostics: OnceLock<Option<Diagnostics>>,
}

// The frames that locate a failure, one per request hop. Each is set once by
// the boundary that owns it, so a frame never repeats.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    operation: Option<String>,
    step: Option<String>,
}

// Renders an error chain on one line for logs and text sinks. Each link prints
// its own line once; a link whose text an outer link already embeds is
// skipped, so a cause is never repeated.
pub struct Report<'a>(pub &'a (dyn error::Error + 'static));

// What the originating process observed: its source chain and, when that
// process enabled capture, the backtrace where its carrier received the error.
// The origin names whose frames these are, so a host capture never poses as a
// plugin's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostics {
    origin: Origin,
    chain: Vec<String>,
    backtrace: Option<String>,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Plugin,
    Host,
}

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("failure encoding has trailing bytes")]
    Trailing,

    #[error("failure encoding is malformed")]
    Encoding(#[source] postcard::Error),
}

/* === Traits === */

/* === Implementations === */

impl Failure {
    /// One Wrong-Caller Failure for Every Carrier.
    pub fn wrong_caller(operation: &str) -> Self {
        Self::from(model::Failure {
            classification: Classification::Refused as i32,
            code: crate::refusal::WRONG_CALLER.to_string(),
            action: "Call".into(),
            operation: operation.into(),
            message: format!("Call {operation}: caller is not allowed"),
            ..Default::default()
        })
    }

    pub fn new(code: FailureKind, cause: impl Into<String>) -> Self {
        let classification = match code {
            FailureKind::Transient => Classification::Transient,
            FailureKind::Cancelled => Classification::Cancelled,
            FailureKind::Invalid | FailureKind::Unauthorized => Classification::Refused,
            _ => Classification::Permanent,
        };
        let kind = match code {
            FailureKind::Permanent => "permanent",
            FailureKind::Transient => "transient",
            FailureKind::Ambiguous => "ambiguous",
            FailureKind::Unauthorized => "unauthorized",
            FailureKind::Invalid => "invalid",
            FailureKind::Cancelled => "cancelled",
        };
        Self::from(model::Failure {
            classification: classification as i32,
            message: cause.into(),
            details: [("harmos.failure_kind".into(), kind.into())].into(),
            ..Default::default()
        })
    }

    // Writes the cause once from an error chain. A failure already carried in
    // the chain keeps its classification, cause, frames and diagnostics; otherwise the deepest
    // meaningful message becomes the cause and the whole chain moves into
    // diagnostics.
    pub fn originate(code: FailureKind, error: &(dyn error::Error + 'static), origin: Origin) -> Self {
        if let Some(carried) = carried(error) {
            return carried.clone();
        }

        let chain: Vec<String> = sources(error)
            .map(|error| match error.downcast_ref::<crate::Refusal>() {
                Some(refusal) => refusal.message().to_owned(),
                None => error.to_string(),
            })
            .collect();
        let cause = chain.iter().rev().find(|message| !message.trim().is_empty()).cloned().unwrap_or_else(|| "operation failed".to_owned());

        let backtrace = Backtrace::capture();
        let backtrace = (backtrace.status() == BacktraceStatus::Captured).then(|| bounded(backtrace.to_string(), BACKTRACE_LIMIT));
        let chain = chain.into_iter().take(CHAIN_LIMIT).map(|message| bounded(message, LINK_LIMIT)).collect();
        let diagnostics = Diagnostics { origin, chain, backtrace };

        let mut failure = Self::new(code, cause);
        failure.0.carrier.diagnostics = serde_json::to_string(&diagnostics).expect("serializable diagnostics");
        failure
    }

    // The failure an error chain carries, if any link is or holds one.
    pub fn find<'a>(error: &'a (dyn error::Error + 'static)) -> Option<&'a Self> {
        carried(error)
    }

    pub fn code(&self) -> FailureKind {
        match self.0.carrier.details.get("harmos.failure_kind").map(String::as_str) {
            Some("ambiguous") => FailureKind::Ambiguous,
            Some("unauthorized") => FailureKind::Unauthorized,
            Some("invalid") => FailureKind::Invalid,
            _ => match self.0.carrier.classification() {
                Classification::Transient => FailureKind::Transient,
                Classification::Cancelled => FailureKind::Cancelled,
                Classification::Refused => FailureKind::Unauthorized,
                Classification::Invalid => FailureKind::Invalid,
                _ => FailureKind::Permanent,
            },
        }
    }

    pub fn refusal_code(&self) -> Option<i64> {
        self.0.carrier.details.get("harmos.refusal_code").unwrap_or(&self.0.carrier.code).parse().ok()
    }

    pub(crate) fn with_refusal_code(mut self, code: i64) -> Self {
        if self.refusal_code().is_none() {
            if self.0.carrier.code.is_empty() {
                self.0.carrier.code = code.to_string();
            } else {
                self.0.carrier.details.insert("harmos.refusal_code".into(), code.to_string());
            }
        }
        self
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.0.carrier.details.insert(key.into(), value.into());
        self.0.context.take();
        self
    }

    pub fn detail(&self, key: &str) -> Option<&str> {
        self.0.carrier.details.get(key).map(String::as_str)
    }

    pub fn cause(&self) -> &str {
        &self.0.carrier.message
    }

    pub fn context(&self) -> &Context {
        self.0.context.get_or_init(|| Context {
            operation: (!self.0.carrier.operation.is_empty()).then(|| self.0.carrier.operation.clone()),
            step: self.0.carrier.details.get("harmos.step").cloned(),
        })
    }

    pub fn diagnostics(&self) -> Option<&Diagnostics> {
        self.0.diagnostics.get_or_init(|| serde_json::from_str(&self.0.carrier.diagnostics).ok()).as_ref()
    }

    // The host adds the operation frame where a call crosses from a plugin.
    // The frame closest to the origin wins, so a forwarded failure keeps it.
    pub fn in_operation(mut self, operation: impl Into<String>) -> Self {
        if self.0.carrier.operation.is_empty() {
            self.0.carrier.operation = operation.into();
            self.0.context.take();
        }
        self
    }

    pub fn without_diagnostics(mut self) -> Self {
        self.0.diagnostics.take();
        let mut current = &mut self.0.carrier;
        loop {
            current.diagnostics.clear();
            let Some(cause) = current.cause.as_deref_mut() else { break };
            current = cause;
        }
        self
    }

    pub fn encode(&self) -> Vec<u8> {
        postcard::to_allocvec(&(VERSION, self)).expect("a failure serializes to postcard")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        let (_version, rest) = postcard::take_from_bytes::<u8>(bytes).map_err(CodecError::Encoding)?;
        let (failure, rest) = postcard::take_from_bytes::<Self>(rest).map_err(CodecError::Encoding)?;
        if !rest.is_empty() {
            return Err(CodecError::Trailing);
        }
        Ok(failure)
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.cause())
    }
}

impl error::Error for Failure {}

impl fmt::Display for Report<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut rendered = String::new();
        for link in sources(self.0) {
            let line = link.to_string();
            if line.trim().is_empty() || rendered.contains(&line) {
                continue;
            }
            if !rendered.is_empty() {
                rendered.push_str(": ");
            }
            rendered.push_str(&line);
        }
        formatter.write_str(&rendered)
    }
}

impl Context {
    pub fn operation(&self) -> Option<&str> {
        self.operation.as_deref()
    }

    pub fn step(&self) -> Option<&str> {
        self.step.as_deref()
    }
}

impl Diagnostics {
    pub fn origin(&self) -> Origin {
        self.origin
    }

    pub fn chain(&self) -> &[String] {
        &self.chain
    }

    pub fn backtrace(&self) -> Option<&str> {
        self.backtrace.as_deref()
    }
}

impl PartialEq for Failure {
    fn eq(&self, other: &Self) -> bool {
        same_carrier(&self.0.carrier, &other.0.carrier)
    }
}

impl Eq for Failure {}

impl From<&Failure> for model::Failure {
    fn from(failure: &Failure) -> Self {
        failure.0.carrier.clone()
    }
}

impl From<model::Failure> for Failure {
    fn from(carrier: model::Failure) -> Self {
        Self(Box::new(Record {
            carrier,
            context: OnceLock::new(),
            diagnostics: OnceLock::new(),
        }))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn same_carrier(left: &model::Failure, right: &model::Failure) -> bool {
    let mut left = Some(left);
    let mut right = Some(right);
    while let (Some(a), Some(b)) = (left, right) {
        let retry = |value: &model::Failure| {
            value.retry.map(|retry| (retry.retryable, retry.attempts, retry.max_attempts, retry.next_after_s.to_bits()))
        };
        if a.classification != b.classification
            || a.code != b.code
            || a.message != b.message
            || a.action != b.action
            || a.operation != b.operation
            || a.plugin != b.plugin
            || a.details != b.details
            || a.diagnostics != b.diagnostics
            || retry(a) != retry(b)
        {
            return false;
        }
        left = a.cause.as_deref();
        right = b.cause.as_deref();
    }
    left.is_none() && right.is_none()
}

fn serialize_carrier<S: Serializer>(carrier: &model::Failure, serializer: S) -> Result<S::Ok, S::Error> {
    carrier.encode_to_vec().serialize(serializer)
}

fn deserialize_carrier<'de, D: Deserializer<'de>>(deserializer: D) -> Result<model::Failure, D::Error> {
    let bytes = Vec::<u8>::deserialize(deserializer)?;
    model::Failure::decode(bytes.as_slice()).map_err(de::Error::custom)
}

fn sources<'a>(error: &'a (dyn error::Error + 'static)) -> impl Iterator<Item = &'a (dyn error::Error + 'static)> {
    iter::successors(Some(error), |error| {
        let shared: Option<&(dyn error::Error + 'static)> = error
            .downcast_ref::<Arc<crate::Refusal>>()
            .map(|inner| inner.as_ref() as _)
            .or_else(|| error.downcast_ref::<Arc<Failure>>().map(|inner| inner.as_ref() as _))
            .or_else(|| error.downcast_ref::<Box<Failure>>().map(|inner| inner.as_ref() as _))
            .or_else(|| error.downcast_ref::<Arc<io::Error>>().map(|inner| inner.as_ref() as _))
            .or_else(|| error.downcast_ref::<Arc<dyn error::Error + Send + Sync>>().map(|inner| inner.as_ref() as _));
        shared.or_else(|| {
            error
                .downcast_ref::<io::Error>()
                .and_then(io::Error::get_ref)
                .map(|inner| inner as &(dyn error::Error + 'static))
                .or_else(|| error.source())
        })
    })
}

fn carried<'a>(error: &'a (dyn error::Error + 'static)) -> Option<&'a Failure> {
    sources(error).find_map(|error| {
        error.downcast_ref::<Failure>().or_else(|| error.downcast_ref::<crate::Refusal>().map(crate::Refusal::failure)).or_else(|| {
            match error.downcast_ref::<crate::execution::ServingError>() {
                Some(crate::execution::ServingError::Handler(failure)) => Some(failure),
                _ => None,
            }
        })
    })
}

fn bounded(mut text: String, limit: usize) -> String {
    if text.len() <= limit {
        return text;
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::{Failure, Origin};
    use crate::{Refusal, operations::FailureKind};
    use harmos_protocol::model::{self, Classification};
    use std::{io, sync::Arc};

    #[derive(Debug, thiserror::Error)]
    #[error("{0}")]
    struct Remote(#[source] Box<Failure>);

    #[test]
    fn carrier_owns_fields_after_cached_views_and_serialization() {
        let original = Failure::originate(FailureKind::Transient, &io::Error::other("Reconnect Sensor"), Origin::Plugin);
        assert!(original.context().operation().is_none());
        assert!(original.diagnostics().is_some());
        let changed = original.clone().in_operation("sensor.read").with_detail("harmos.step", "read").without_diagnostics();
        assert!(original.context().operation().is_none());
        assert!(original.diagnostics().is_some());
        assert_eq!(changed.context().operation(), Some("sensor.read"));
        assert_eq!(changed.context().step(), Some("read"));
        assert!(changed.diagnostics().is_none());
        let wire = model::Failure::from(&changed);
        assert_eq!(Failure::from(wire), changed);
        assert_eq!(Failure::decode(&changed.encode()).unwrap(), changed);
        assert_eq!(serde_json::from_str::<Failure>(&serde_json::to_string(&changed).unwrap()).unwrap(), changed);
    }

    #[test]
    fn invalid_wire_payload_preserves_its_classification_and_action() {
        let wire = model::Failure {
            classification: Classification::Invalid as i32,
            operation: "fixture.echo".into(),
            message: "Call fixture.echo: Field \"radio\" Must Be String".into(),
            action: "Supply a String".into(),
            ..Default::default()
        };
        let received = Failure::from(wire.clone());
        assert_eq!(received.code(), FailureKind::Invalid);
        assert_eq!(model::Failure::from(&received), wire);
    }

    #[test]
    fn application_failure_survives_origin_and_carrier_roundtrip() {
        let source = io::Error::from(io::ErrorKind::NotFound);
        let original = Failure::originate(FailureKind::Transient, &source, Origin::Plugin)
            .with_detail("plugin", "fixture")
            .with_detail("secret", "api_token");
        let refusal = Refusal::failed(-32_100, original);
        let wrapped = Arc::new(io::Error::other(refusal));
        let forwarded = Failure::originate(FailureKind::Permanent, &wrapped, Origin::Host).in_operation("harmos.initialize");
        let received = Failure::decode(&forwarded.encode()).unwrap();
        assert_eq!(received, forwarded);
        assert_eq!(received.code(), FailureKind::Transient);
        assert_eq!(received.refusal_code(), Some(-32_100));
        assert_eq!(received.detail("plugin"), Some("fixture"));
        assert_eq!(received.detail("secret"), Some("api_token"));
        assert_eq!(received.context().operation(), Some("harmos.initialize"));
        assert_eq!(received.diagnostics().unwrap().origin(), Origin::Plugin);
        assert_eq!(received.diagnostics().unwrap().chain().len(), 1);
        let wire = model::Failure {
            classification: Classification::Transient as i32,
            code: "sensor_unavailable".into(),
            message: "sensor unavailable".into(),
            action: "Reconnect Sensor".into(),
            plugin: "sensor".into(),
            diagnostics: "plugin diagnostic".into(),
            cause: Some(Box::new(model::Failure {
                message: "link closed".into(),
                diagnostics: "cause diagnostic".into(),
                ..Default::default()
            })),
            ..Default::default()
        };
        let received = Failure::from(wire.clone()).in_operation("sensor.read");
        let remote = Remote(Box::new(received.clone()));
        assert_eq!(Failure::find(&remote), Some(&received));
        let retained = Failure::decode(&received.encode()).unwrap();
        let encoded = model::Failure::from(&retained);
        assert_eq!(encoded.code, wire.code);
        assert_eq!(encoded.action, wire.action);
        assert_eq!(encoded.cause, wire.cause);
        assert_eq!(encoded.diagnostics, wire.diagnostics);
        assert_eq!(encoded.plugin, wire.plugin);
        assert_eq!(encoded.details, wire.details);
        assert_eq!(encoded.operation, "sensor.read");
        let redacted = retained.without_diagnostics();
        let redacted = Failure::decode(&redacted.encode()).unwrap();
        assert!(redacted.diagnostics().is_none());
        let redacted = model::Failure::from(&redacted);
        assert!(redacted.diagnostics.is_empty());
        assert!(redacted.cause.unwrap().diagnostics.is_empty());

        // Transport Refusals Preserve the Origin's Symbolic Failure Code.
        let refusal = Refusal::failed(-32_100, Failure::from(wire));
        assert_eq!(refusal.failure().refusal_code(), Some(-32_100));
        let retained = Failure::decode(&refusal.failure().encode()).unwrap();
        assert_eq!(model::Failure::from(&retained).code, "sensor_unavailable");
        assert_eq!(retained.refusal_code(), Some(-32_100));
    }
}
