/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod payload {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<&'de [u8], D::Error> {
        <&[u8]>::deserialize(deserializer)
    }
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::{Failure, WireAnswer, WireChildCall, WireCommand};

/* === Constants === */
const VERSION: u8 = 4;
pub(super) const LIMIT: usize = 16 * 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Serialize, Deserialize)]
struct Value<'a> {
    key: &'a str,
    #[serde(borrow, with = "payload")]
    payload: &'a [u8],
}

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum WireCodecError {
    #[error("operation envelope exceeds the 16 MiB limit")]
    Oversized,
    #[error("unsupported operation envelope version")]
    Version,
    #[error("unexpected operation envelope kind")]
    Kind,
    #[error("trailing operation envelope bytes")]
    Trailing,
    #[error("invalid operation envelope")]
    Encoding(#[from] postcard::Error),
}

// Variant Order Is Part Of Version 4. Borrowed Fields Validate Before Allocation.
// As In The Transport Experiment, Payloads Use Postcard Byte Strings, Not Sequences.
#[derive(Serialize, Deserialize)]
enum Envelope<'a> {
    Prepare {
        invocation: u64,
        operation: &'a str,
        #[serde(borrow, with = "payload")]
        payload: &'a [u8],
    },
    Initialize {
        invocation: u64,
        context: [u8; 32],
    },
    Attempt {
        invocation: u64,
        context: [u8; 32],
        #[serde(borrow)]
        values: Vec<Value<'a>>,
    },
    Finalize {
        invocation: u64,
    },
    Ready {
        #[serde(borrow, with = "payload")]
        payload: &'a [u8],
    },
    Failed {
        failure: Failure,
        message: &'a str,
    },
    Child {
        context: [u8; 32],
        generation_context: Option<[u8; 32]>,
        operation: &'a str,
        #[serde(borrow, with = "payload")]
        payload: &'a [u8],
    },
    PrepareGrpc {
        invocation: u64,
        operation: &'a str,
        #[serde(borrow, with = "payload")]
        payload: &'a [u8],
    },
    StreamAttempt {
        invocation: u64,
        epoch: u64,
        context: [u8; 32],
        #[serde(borrow)]
        values: Vec<Value<'a>>,
    },
    StreamSend {
        invocation: u64,
        epoch: u64,
        #[serde(borrow, with = "payload")]
        payload: &'a [u8],
    },
    StreamFinish {
        invocation: u64,
        epoch: u64,
    },
    StreamReceive {
        invocation: u64,
        epoch: u64,
    },
    StreamItem {
        epoch: u64,
        transferred: bool,
        #[serde(borrow, with = "payload")]
        payload: &'a [u8],
    },
    StreamEnd {
        epoch: u64,
        transferred: bool,
    },
}

/* === Traits === */

/* === Implementations === */
impl WireCommand {
    pub fn encode(&self) -> Result<Vec<u8>, WireCodecError> {
        encode(self.envelope())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireCodecError> {
        Ok(match decode(bytes)? {
            Envelope::Prepare {
                invocation,
                operation,
                payload,
            } => Self::Prepare {
                invocation,
                operation: operation.to_owned(),
                payload: payload.to_vec(),
            },
            Envelope::PrepareGrpc {
                invocation,
                operation,
                payload,
            } => Self::PrepareGrpc {
                invocation,
                operation: operation.to_owned(),
                payload: payload.to_vec(),
            },
            Envelope::Initialize {
                invocation,
                context,
            } => Self::Initialize {
                invocation,
                context,
            },
            Envelope::Attempt {
                invocation,
                context,
                values,
            } => Self::Attempt {
                invocation,
                context,
                values: owned(values)?,
            },
            Envelope::Finalize { invocation } => Self::Finalize { invocation },
            Envelope::StreamAttempt {
                invocation,
                epoch,
                context,
                values,
            } => Self::StreamAttempt {
                invocation,
                epoch,
                context,
                values: owned(values)?,
            },
            Envelope::StreamSend {
                invocation,
                epoch,
                payload,
            } => Self::StreamSend {
                invocation,
                epoch,
                payload: payload.to_vec(),
            },
            Envelope::StreamFinish { invocation, epoch } => {
                Self::StreamFinish { invocation, epoch }
            }
            Envelope::StreamReceive { invocation, epoch } => {
                Self::StreamReceive { invocation, epoch }
            }
            _ => return Err(WireCodecError::Kind),
        })
    }

    fn envelope(&self) -> Envelope<'_> {
        match self {
            Self::Prepare {
                invocation,
                operation,
                payload,
            } => Envelope::Prepare {
                invocation: *invocation,
                operation,
                payload,
            },
            Self::PrepareGrpc {
                invocation,
                operation,
                payload,
            } => Envelope::PrepareGrpc {
                invocation: *invocation,
                operation,
                payload,
            },
            Self::Initialize {
                invocation,
                context,
            } => Envelope::Initialize {
                invocation: *invocation,
                context: *context,
            },
            Self::Attempt {
                invocation,
                context,
                values,
            } => Envelope::Attempt {
                invocation: *invocation,
                context: *context,
                values: borrowed(values),
            },
            Self::StreamAttempt {
                invocation,
                epoch,
                context,
                values,
            } => Envelope::StreamAttempt {
                invocation: *invocation,
                epoch: *epoch,
                context: *context,
                values: borrowed(values),
            },
            Self::StreamSend {
                invocation,
                epoch,
                payload,
            } => Envelope::StreamSend {
                invocation: *invocation,
                epoch: *epoch,
                payload,
            },
            Self::StreamFinish { invocation, epoch } => Envelope::StreamFinish {
                invocation: *invocation,
                epoch: *epoch,
            },
            Self::StreamReceive { invocation, epoch } => Envelope::StreamReceive {
                invocation: *invocation,
                epoch: *epoch,
            },
            Self::Finalize { invocation } => Envelope::Finalize {
                invocation: *invocation,
            },
        }
    }
}

impl WireAnswer {
    pub fn encode(&self) -> Result<Vec<u8>, WireCodecError> {
        let envelope = match self {
            Self::Ready { payload } => Envelope::Ready { payload },
            Self::Stream {
                epoch,
                transferred,
                payload: Some(payload),
            } => Envelope::StreamItem {
                epoch: *epoch,
                transferred: *transferred,
                payload,
            },
            Self::Stream {
                epoch,
                transferred,
                payload: None,
            } => Envelope::StreamEnd {
                epoch: *epoch,
                transferred: *transferred,
            },
            Self::Failed { failure, message } => Envelope::Failed {
                failure: *failure,
                message,
            },
        };
        encode(envelope)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireCodecError> {
        Ok(match decode(bytes)? {
            Envelope::StreamItem {
                epoch,
                transferred,
                payload,
            } => Self::Stream {
                epoch,
                transferred,
                payload: Some(payload.to_vec()),
            },
            Envelope::StreamEnd { epoch, transferred } => Self::Stream {
                epoch,
                transferred,
                payload: None,
            },
            Envelope::Ready { payload } => Self::Ready {
                payload: payload.to_vec(),
            },
            Envelope::Failed { failure, message } => Self::Failed {
                failure,
                message: message.to_owned(),
            },
            _ => return Err(WireCodecError::Kind),
        })
    }
}

impl WireChildCall {
    pub fn encode(&self) -> Result<Vec<u8>, WireCodecError> {
        encode(Envelope::Child {
            context: self.context,
            generation_context: self.generation_context,
            operation: &self.operation,
            payload: &self.payload,
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireCodecError> {
        let Envelope::Child {
            context,
            generation_context,
            operation,
            payload,
        } = decode(bytes)?
        else {
            return Err(WireCodecError::Kind);
        };
        Ok(Self {
            context,
            generation_context,
            operation: operation.to_owned(),
            payload: payload.to_vec(),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn borrowed(values: &[super::super::CallValue]) -> Vec<Value<'_>> {
    values
        .iter()
        .map(|value| Value {
            key: &value.key,
            payload: &value.payload,
        })
        .collect()
}

fn owned(values: Vec<Value<'_>>) -> Result<Vec<super::super::CallValue>, WireCodecError> {
    let bytes = values
        .iter()
        .map(|value| value.key.len() + value.payload.len())
        .sum::<usize>();
    if values.len() > super::super::PORTABLE_VALUES || bytes > super::super::PORTABLE_BYTES {
        return Err(WireCodecError::Oversized);
    }
    Ok(values
        .into_iter()
        .map(|value| super::super::CallValue {
            key: value.key.to_owned(),
            payload: value.payload.to_vec(),
        })
        .collect())
}

fn encode(envelope: Envelope<'_>) -> Result<Vec<u8>, WireCodecError> {
    let message = (VERSION, envelope);
    let size = postcard::experimental::serialized_size(&message)?;
    if size > LIMIT {
        return Err(WireCodecError::Oversized);
    }
    Ok(postcard::to_allocvec(&message)?)
}

fn decode(bytes: &[u8]) -> Result<Envelope<'_>, WireCodecError> {
    if bytes.len() > LIMIT {
        return Err(WireCodecError::Oversized);
    }
    let (version, rest) = postcard::take_from_bytes::<u8>(bytes)?;
    if version != VERSION {
        return Err(WireCodecError::Version);
    }
    let (envelope, rest) = postcard::take_from_bytes::<Envelope<'_>>(rest)?;
    if !rest.is_empty() {
        return Err(WireCodecError::Trailing);
    }
    Ok(envelope)
}

/* === Tests === */
