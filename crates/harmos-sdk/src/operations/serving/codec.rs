/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod payload {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<&'de [u8], D::Error> {
        <&[u8]>::deserialize(deserializer)
    }
}

use serde::{Deserialize, Serialize};

use super::{WireAnswer, WireChild, WireChildAction, WireChildCall, WireChildControl, WireCommand};

const VERSION: u8 = 1;
pub(super) const LIMIT: usize = 16 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Value<'a> {
    key: &'a str,

    #[serde(borrow, with = "payload")]
    payload: &'a [u8],
}

#[derive(Debug, thiserror::Error)]
pub enum WireCodecError {
    #[error("operation envelope exceeds the 16 MiB limit")]
    Oversized,

    #[error("unexpected operation envelope kind")]
    Kind,

    #[error("trailing operation envelope bytes")]
    Trailing,

    #[error("invalid operation envelope")]
    Encoding(#[from] postcard::Error),
}

// Variant Order Is Part Of Version 1. Borrowed Fields Validate Before Allocation.
// As In The Transport Experiment, Payloads Use Postcard Byte Strings, Not Sequences.
#[derive(Serialize, Deserialize)]
enum Envelope<'a> {
    Prepare {
        invocation: u64,
        operation: &'a str,
        fingerprint: [u8; 32],
        generation: u64,

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
        failure: crate::Failure,
    },
    Child {
        context: [u8; 32],
        generation_context: Option<[u8; 32]>,

        #[serde(borrow)]
        values: Vec<Value<'a>>,

        operation: &'a str,
        fingerprint: [u8; 32],
        generation: u64,

        #[serde(borrow, with = "payload")]
        payload: &'a [u8],

        submit: bool,
    },
    PrepareGrpc {
        invocation: u64,
        operation: &'a str,
        fingerprint: [u8; 32],
        generation: u64,

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
    ChildControl {
        context: [u8; 32],
        instance: u64,
        action: WireChildAction,
    },
}

impl WireCommand {
    /// Counts the versioned Prepare envelope without encoding or allocating its payload.
    pub fn prepare_encoded_len(
        invocation: u64, operation: &str, fingerprint: [u8; 32], generation: u64, payload: usize,
    ) -> Result<usize, WireCodecError> {
        payload_size(
            Envelope::Prepare {
                invocation,
                operation,
                fingerprint,
                generation,
                payload: &[],
            },
            payload,
        )
    }

    pub fn encode(&self) -> Result<Vec<u8>, WireCodecError> {
        encode(self.envelope())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireCodecError> {
        Ok(match decode(bytes)? {
            Envelope::Prepare {
                invocation,
                operation,
                fingerprint,
                generation,
                payload,
            } => Self::Prepare {
                invocation,
                operation: operation.to_owned(),
                fingerprint,
                generation,
                payload: payload.to_vec(),
            },
            Envelope::PrepareGrpc {
                invocation,
                operation,
                fingerprint,
                generation,
                payload,
            } => Self::PrepareGrpc {
                invocation,
                operation: operation.to_owned(),
                fingerprint,
                generation,
                payload: payload.to_vec(),
            },
            Envelope::Initialize { invocation, context } => Self::Initialize { invocation, context },
            Envelope::Attempt { invocation, context, values } => Self::Attempt {
                invocation,
                context,
                values: owned(values)?,
            },
            Envelope::Finalize { invocation } => Self::Finalize { invocation },
            Envelope::StreamAttempt { invocation, epoch, context, values } => Self::StreamAttempt {
                invocation,
                epoch,
                context,
                values: owned(values)?,
            },
            Envelope::StreamSend { invocation, epoch, payload } => Self::StreamSend {
                invocation,
                epoch,
                payload: payload.to_vec(),
            },
            Envelope::StreamFinish { invocation, epoch } => Self::StreamFinish { invocation, epoch },
            Envelope::StreamReceive { invocation, epoch } => Self::StreamReceive { invocation, epoch },
            _ => return Err(WireCodecError::Kind),
        })
    }

    fn envelope(&self) -> Envelope<'_> {
        match self {
            Self::Prepare {
                invocation,
                operation,
                fingerprint,
                generation,
                payload,
            } => Envelope::Prepare {
                invocation: *invocation,
                operation,
                fingerprint: *fingerprint,
                generation: *generation,
                payload,
            },
            Self::PrepareGrpc {
                invocation,
                operation,
                fingerprint,
                generation,
                payload,
            } => Envelope::PrepareGrpc {
                invocation: *invocation,
                operation,
                fingerprint: *fingerprint,
                generation: *generation,
                payload,
            },
            Self::Initialize { invocation, context } => Envelope::Initialize {
                invocation: *invocation,
                context: *context,
            },
            Self::Attempt { invocation, context, values } => Envelope::Attempt {
                invocation: *invocation,
                context: *context,
                values: borrowed(values),
            },
            Self::StreamAttempt { invocation, epoch, context, values } => Envelope::StreamAttempt {
                invocation: *invocation,
                epoch: *epoch,
                context: *context,
                values: borrowed(values),
            },
            Self::StreamSend { invocation, epoch, payload } => Envelope::StreamSend {
                invocation: *invocation,
                epoch: *epoch,
                payload,
            },
            Self::StreamFinish { invocation, epoch } => Envelope::StreamFinish { invocation: *invocation, epoch: *epoch },
            Self::StreamReceive { invocation, epoch } => Envelope::StreamReceive { invocation: *invocation, epoch: *epoch },
            Self::Finalize { invocation } => Envelope::Finalize { invocation: *invocation },
        }
    }
}

impl WireAnswer {
    /// Counts the versioned Ready envelope without encoding or allocating its payload.
    pub fn ready_encoded_len(payload: usize) -> Result<usize, WireCodecError> {
        payload_size(Envelope::Ready { payload: &[] }, payload)
    }

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
            Self::Stream { epoch, transferred, payload: None } => Envelope::StreamEnd { epoch: *epoch, transferred: *transferred },
            Self::Failed { failure } => Envelope::Failed { failure: failure.clone() },
        };
        encode(envelope)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireCodecError> {
        Ok(match decode(bytes)? {
            Envelope::StreamItem { epoch, transferred, payload } => Self::Stream {
                epoch,
                transferred,
                payload: Some(payload.to_vec()),
            },
            Envelope::StreamEnd { epoch, transferred } => Self::Stream { epoch, transferred, payload: None },
            Envelope::Ready { payload } => Self::Ready { payload: payload.to_vec() },
            Envelope::Failed { failure } => Self::Failed { failure },
            _ => return Err(WireCodecError::Kind),
        })
    }
}

impl WireChildCall {
    /// Counts a Child envelope with its actual portable values and payload length.
    /// Context identifiers and the submit flag have fixed encoded widths.
    pub fn payload_encoded_len(
        operation: &str, generation: u64, payload: usize, generation_context: bool, values: &[super::super::CallValue],
    ) -> Result<usize, WireCodecError> {
        check_values(values)?;
        let envelope = Envelope::Child {
            context: [0; 32],
            generation_context: generation_context.then_some([0; 32]),
            values: Vec::new(),
            operation,
            fingerprint: [0; 32],
            generation,
            payload: &[],
            submit: false,
        };
        let count = postcard::experimental::serialized_size(&values.len())?;
        let mut size = add_size(payload_size(envelope, payload)?, count - 1)?;
        for value in values {
            let value = Value { key: &value.key, payload: &value.payload };
            size = add_size(size, postcard::experimental::serialized_size(&value)?)?;
        }
        Ok(size)
    }

    pub fn encode(&self) -> Result<Vec<u8>, WireCodecError> {
        check_values(&self.values)?;
        encode(Envelope::Child {
            context: self.context,
            generation_context: self.generation_context,
            values: borrowed(&self.values),
            operation: &self.operation,
            fingerprint: self.fingerprint,
            generation: self.generation,
            payload: &self.payload,
            submit: self.submit,
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireCodecError> {
        match WireChild::decode(bytes)? {
            WireChild::Call(call) => Ok(call),
            WireChild::Control(_) => Err(WireCodecError::Kind),
        }
    }
}

impl WireChildControl {
    pub fn encode(&self) -> Result<Vec<u8>, WireCodecError> {
        encode(Envelope::ChildControl {
            context: self.context,
            instance: self.instance,
            action: self.action,
        })
    }
}

impl WireChild {
    pub fn decode(bytes: &[u8]) -> Result<Self, WireCodecError> {
        let child = match decode(bytes)? {
            Envelope::Child {
                context,
                generation_context,
                values,
                operation,
                fingerprint,
                generation,
                payload,
                submit,
            } => Self::Call(WireChildCall {
                context,
                generation_context,
                values: owned(values)?,
                operation: operation.to_owned(),
                fingerprint,
                generation,
                payload: payload.to_vec(),
                submit,
            }),
            Envelope::ChildControl { context, instance, action } => Self::Control(WireChildControl { context, instance, action }),
            _ => return Err(WireCodecError::Kind),
        };
        Ok(child)
    }

    pub fn context(&self) -> &[u8; 32] {
        match self {
            Self::Call(call) => &call.context,
            Self::Control(control) => &control.context,
        }
    }
}

fn check_values(values: &[super::super::CallValue]) -> Result<(), WireCodecError> {
    let bytes = values.iter().map(|value| value.key.len().saturating_add(value.payload.len())).fold(0usize, usize::saturating_add);
    if values.len() > super::super::PORTABLE_VALUES || bytes > super::super::PORTABLE_BYTES {
        return Err(WireCodecError::Oversized);
    }
    for (index, value) in values.iter().enumerate() {
        if values[..index].iter().any(|previous| previous.key == value.key) {
            return Err(WireCodecError::Kind);
        }
    }
    Ok(())
}

fn borrowed(values: &[super::super::CallValue]) -> Vec<Value<'_>> {
    values.iter().map(|value| Value { key: &value.key, payload: &value.payload }).collect()
}

fn owned(values: Vec<Value<'_>>) -> Result<Vec<super::super::CallValue>, WireCodecError> {
    let bytes = values.iter().map(|value| value.key.len() + value.payload.len()).sum::<usize>();
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

fn payload_size(envelope: Envelope<'_>, payload: usize) -> Result<usize, WireCodecError> {
    let empty = postcard::experimental::serialized_size(&(VERSION, envelope))?;
    let prefix = postcard::experimental::serialized_size(&payload)?;
    add_size(add_size(empty, payload)?, prefix - 1)
}

fn add_size(left: usize, right: usize) -> Result<usize, WireCodecError> {
    left.checked_add(right).ok_or(WireCodecError::Oversized)
}

fn decode(bytes: &[u8]) -> Result<Envelope<'_>, WireCodecError> {
    if bytes.len() > LIMIT {
        return Err(WireCodecError::Oversized);
    }
    let (_version, rest) = postcard::take_from_bytes::<u8>(bytes)?;
    let (envelope, rest) = postcard::take_from_bytes::<Envelope<'_>>(rest)?;
    if !rest.is_empty() {
        return Err(WireCodecError::Trailing);
    }
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::{CallValue, PORTABLE_BYTES, PORTABLE_VALUES};

    #[test]
    fn envelope_sizes_match() {
        let sizes = [0, 1, 127, 128, 16383, 16384, 2097151, 2097152];
        let ids = [0, 127, 128, u64::MAX];
        for (index, size) in sizes.into_iter().enumerate() {
            let operation = "λ".repeat([1, 63, 64, 65][index % 4]);
            let invocation = ids[index % ids.len()];
            let payload = vec![255; size];
            let expected = WireCommand::prepare_encoded_len(invocation, &operation, [255; 32], invocation, size).unwrap();
            let command = WireCommand::Prepare {
                invocation,
                operation,
                fingerprint: [255; 32],
                generation: invocation,
                payload: payload.clone(),
            };
            assert_eq!(command.encode().unwrap().len(), expected);
            let expected = WireAnswer::ready_encoded_len(size).unwrap();
            assert_eq!(WireAnswer::Ready { payload }.encode().unwrap().len(), expected);
        }
        assert!(WireCommand::prepare_encoded_len(0, "op", [255; 32], u64::MAX, usize::MAX).is_err());
        assert!(WireAnswer::ready_encoded_len(usize::MAX).is_err());
    }

    #[test]
    fn prepare_identity_roundtrips() {
        let command = WireCommand::Prepare {
            invocation: 127,
            operation: "fixture.read".into(),
            fingerprint: [173; 32],
            generation: u64::MAX,
            payload: vec![1, 2],
        };
        let bytes = command.encode().unwrap();
        assert!(matches!(WireCommand::decode(&bytes).unwrap(), WireCommand::Prepare {
            invocation: 127, operation, fingerprint, generation: u64::MAX, payload
        } if operation == "fixture.read" && fingerprint == [173; 32] && payload == [1, 2]));
    }

    #[test]
    fn child_values_count() {
        let mut maximal: Vec<_> = (0..PORTABLE_VALUES)
            .map(|index| CallValue {
                key: format!("k{index:02}"),
                payload: Vec::new(),
            })
            .collect();
        maximal[0].payload = vec![255; PORTABLE_BYTES - PORTABLE_VALUES * 3];
        for values in [
            Vec::new(),
            vec![CallValue {
                key: "測定".into(),
                payload: vec![255; 128],
            }],
            maximal,
        ] {
            for generation in [false, true] {
                for submit in [false, true] {
                    let expected = WireChildCall::payload_encoded_len("op.λ", u64::MAX, 16384, generation, &values).unwrap();
                    let call = WireChildCall {
                        context: [255; 32],
                        generation_context: generation.then_some([255; 32]),
                        values: values.clone(),
                        operation: "op.λ".into(),
                        fingerprint: [1; 32],
                        generation: u64::MAX,
                        payload: vec![255; 16384],
                        submit,
                    };
                    assert_eq!(call.encode().unwrap().len(), expected);
                }
            }
        }
    }

    #[test]
    fn child_validation_matches() {
        let invalid = [
            vec![CallValue {
                key: "key".into(),
                payload: vec![0; PORTABLE_BYTES],
            }],
            vec![CallValue { key: "same".into(), payload: Vec::new() }; 2],
            (0..=PORTABLE_VALUES)
                .map(|index| CallValue {
                    key: index.to_string(),
                    payload: Vec::new(),
                })
                .collect(),
        ];
        for values in invalid {
            assert!(WireChildCall::payload_encoded_len("op", u64::MAX, 0, false, &values).is_err());
            let call = WireChildCall {
                context: [0; 32],
                generation_context: None,
                values,
                operation: "op".into(),
                fingerprint: [1; 32],
                generation: u64::MAX,
                payload: Vec::new(),
                submit: false,
            };
            assert!(call.encode().is_err());
        }
        assert!(WireChildCall::payload_encoded_len("op", u64::MAX, usize::MAX, false, &[]).is_err());
    }
}
