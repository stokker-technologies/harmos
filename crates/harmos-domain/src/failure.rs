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
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::operations;

/* === Constants === */

const LINK_LIMIT: usize = 4 * 1024;

const CHAIN_LIMIT: usize = 16;

const BACKTRACE_LIMIT: usize = 32 * 1024;

const VERSION: u8 = 1;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// Boxed so every error enum that carries a failure stays one pointer wide.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Failure(Box<Record>);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    code: operations::FailureKind,
    refusal_code: Option<i64>,
    details: BTreeMap<String, String>,
    cause: String,
    context: Context,
    diagnostics: Option<Diagnostics>,
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
pub struct Report<'a>(pub &'a (dyn std::error::Error + 'static));

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
    pub fn new(code: operations::FailureKind, cause: impl Into<String>) -> Self {
        Self(Box::new(Record {
            code,
            refusal_code: None,
            details: BTreeMap::new(),
            cause: cause.into(),
            context: Context::default(),
            diagnostics: None,
        }))
    }

    // Writes the cause once from an error chain. A failure already carried in
    // the chain keeps its classification, cause, frames and diagnostics; otherwise the deepest
    // meaningful message becomes the cause and the whole chain moves into
    // diagnostics.
    pub fn originate(code: operations::FailureKind, error: &(dyn std::error::Error + 'static), origin: Origin) -> Self {
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
        failure.0.diagnostics = Some(diagnostics);
        failure
    }

    // The failure an error chain carries, if any link is or holds one.
    pub fn find<'a>(error: &'a (dyn std::error::Error + 'static)) -> Option<&'a Self> {
        carried(error)
    }

    pub fn code(&self) -> operations::FailureKind {
        self.0.code
    }

    pub fn refusal_code(&self) -> Option<i64> {
        self.0.refusal_code
    }

    pub(crate) fn with_refusal_code(mut self, code: i64) -> Self {
        self.0.refusal_code.get_or_insert(code);
        self
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.0.details.insert(key.into(), value.into());
        self
    }

    pub fn detail(&self, key: &str) -> Option<&str> {
        self.0.details.get(key).map(String::as_str)
    }

    pub fn cause(&self) -> &str {
        &self.0.cause
    }

    pub fn context(&self) -> &Context {
        &self.0.context
    }

    pub fn diagnostics(&self) -> Option<&Diagnostics> {
        self.0.diagnostics.as_ref()
    }

    // The host adds the operation frame where a call crosses from a plugin.
    // The frame closest to the origin wins, so a forwarded failure keeps it.
    pub fn in_operation(mut self, operation: impl Into<String>) -> Self {
        self.0.context.operation.get_or_insert_with(|| operation.into());
        self
    }

    pub fn without_diagnostics(mut self) -> Self {
        self.0.diagnostics = None;
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
        formatter.write_str(&self.0.cause)
    }
}

impl std::error::Error for Failure {}

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

/* === Public Functions === */

/* === Private Functions === */

fn sources<'a>(error: &'a (dyn std::error::Error + 'static)) -> impl Iterator<Item = &'a (dyn std::error::Error + 'static)> {
    std::iter::successors(Some(error), |error| {
        let shared: Option<&(dyn std::error::Error + 'static)> = error
            .downcast_ref::<Arc<crate::Refusal>>()
            .map(|inner| inner.as_ref() as _)
            .or_else(|| error.downcast_ref::<Arc<Failure>>().map(|inner| inner.as_ref() as _))
            .or_else(|| error.downcast_ref::<Arc<std::io::Error>>().map(|inner| inner.as_ref() as _))
            .or_else(|| error.downcast_ref::<Arc<dyn std::error::Error + Send + Sync>>().map(|inner| inner.as_ref() as _));
        shared.or_else(|| {
            error
                .downcast_ref::<std::io::Error>()
                .and_then(std::io::Error::get_ref)
                .map(|inner| inner as &(dyn std::error::Error + 'static))
                .or_else(|| error.source())
        })
    })
}

fn carried<'a>(error: &'a (dyn std::error::Error + 'static)) -> Option<&'a Failure> {
    sources(error)
        .find_map(|error| error.downcast_ref::<Failure>().or_else(|| error.downcast_ref::<crate::Refusal>().map(crate::Refusal::failure)))
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

    #[test]
    fn application_failure_survives_origin_and_carrier_roundtrip() {
        let source = std::io::Error::from(std::io::ErrorKind::NotFound);
        let original = Failure::originate(FailureKind::Transient, &source, Origin::Plugin)
            .with_detail("plugin", "fixture")
            .with_detail("secret", "api_token");
        let refusal = Refusal::failed(-32_100, original);
        let wrapped = std::sync::Arc::new(std::io::Error::other(refusal));
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
    }
}
