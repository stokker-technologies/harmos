/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::ops::{Deref, Range};
use std::sync::Arc;

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};

use harmos_domain::Message;
use harmos_sdk::operations::{CHILD_METHOD, CallValue, OperationDeclaration, WIRE_METHOD, WireAnswer, WireChildCall, WireCommand};

#[derive(Clone, Default)]
pub struct FramePayload {
    bytes: Arc<Vec<u8>>,
    range: Range<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCatalog {
    pub registrations: Vec<crate::Registration>,
    pub observers: Vec<crate::inventory::observers::Declaration>,
    pub operations: Vec<crate::operations::OperationDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Publication {
    pub stream: String,
    pub schema: u32,
    pub sequence: u64,
    pub published_unix_micros: Option<i64>,
    pub context: Option<String>,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
    Call { id: u64, method: String, params: FramePayload },
    Cancel { id: u64 },
    Push { method: String, params: FramePayload },
    Answer { id: u64, result: FramePayload },
    Refusal { id: u64, refusal: harmos_domain::Refusal },
}

impl FramePayload {
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[self.range.clone()]
    }

    /// Compacts a unique backing allocation into its visible bytes.
    /// Shared payloads copy their visible bytes into an independent allocation.
    pub fn into_vec(self) -> Vec<u8> {
        let Self { bytes, range } = self;
        let mut bytes = match Arc::try_unwrap(bytes) {
            Ok(bytes) => bytes,
            Err(bytes) => return bytes[range].to_vec(),
        };
        let length = range.len();
        if range.start != 0 {
            bytes.copy_within(range, 0);
        }
        bytes.truncate(length);
        bytes
    }

    pub(crate) fn from_range(bytes: Vec<u8>, range: Range<usize>) -> Self {
        assert!(range.start <= range.end && range.end <= bytes.len());
        Self { bytes: Arc::new(bytes), range }
    }
}

impl From<Vec<u8>> for FramePayload {
    fn from(bytes: Vec<u8>) -> Self {
        let range = 0..bytes.len();
        Self::from_range(bytes, range)
    }
}

impl Deref for FramePayload {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl AsRef<[u8]> for FramePayload {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl<T: AsRef<[u8]> + ?Sized> PartialEq<T> for FramePayload {
    fn eq(&self, other: &T) -> bool {
        self.as_slice() == other.as_ref()
    }
}

impl Eq for FramePayload {}

impl harmos_domain::MemorySize for FramePayload {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(std::mem::size_of::<Vec<u8>>())
            .saturating_add(2 * std::mem::size_of::<usize>())
            .saturating_add(self.bytes.capacity())
    }
}

impl std::fmt::Debug for FramePayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_slice().fmt(formatter)
    }
}

impl Frame {
    pub const REGISTER: &'static str = "harmos.register";
    pub const EXITING: &'static str = "harmos.exiting";
    pub const LIMIT: usize = 16 * 1024 * 1024;
    pub const OVERSIZED: i64 = -32_603;

    /// Bounds a routed Prepare or child call with no portable context values.
    /// Uses maximal identifiers and includes a generation context for the child.
    /// Frame lengths exclude the four-byte physical packet prefix.
    pub fn request_encoded_len<O: OperationDeclaration>(request: &O::Request) -> usize {
        Self::request_payload_encoded_len(O::ID, harmos_domain::Message::encoded_size(request))
    }

    /// Bounds a routed Prepare or empty-values child carrying this encoded payload.
    /// Use child_payload_encoded_len when the child carries portable values.
    pub fn request_payload_encoded_len(operation: &str, payload: usize) -> usize {
        if operation.len() > Self::LIMIT {
            return usize::MAX;
        }
        let prepare = WireCommand::prepare_encoded_len(u64::MAX, operation, [255; 32], u64::MAX, payload).unwrap_or(usize::MAX);
        call_encoded_len(u64::MAX, WIRE_METHOD, prepare).max(Self::child_payload_encoded_len(operation, u64::MAX, payload, true, &[]))
    }

    /// Bounds a child call using its actual portable values and generation presence.
    /// Only the unknown outer identifier is bounded by its maximal encoded width.
    pub fn child_payload_encoded_len(
        operation: &str, generation: u64, payload: usize, generation_context: bool, values: &[CallValue],
    ) -> usize {
        let child = WireChildCall::payload_encoded_len(operation, generation, payload, generation_context, values).unwrap_or(usize::MAX);
        call_encoded_len(u64::MAX, CHILD_METHOD, child)
    }

    pub(crate) fn child_encoded_from_empty(empty_envelope: usize, payload: usize) -> usize {
        let growth = binary_bytes_len(payload).saturating_sub(1);
        call_encoded_len(u64::MAX, CHILD_METHOD, empty_envelope.saturating_add(growth))
    }

    /// Tests the routed Prepare or empty-values child bound; not an arbitrary child context.
    pub fn request_fits<O: OperationDeclaration>(request: &O::Request) -> bool {
        Self::request_encoded_len::<O>(request) <= Self::LIMIT
    }

    pub fn reply_encoded_len<O: OperationDeclaration>(reply: &O::Response) -> usize {
        let payload = WireAnswer::ready_encoded_len(reply.encoded_size()).unwrap_or(usize::MAX);
        answer_encoded_len(u64::MAX, payload)
    }

    pub fn reply_fits<O: OperationDeclaration>(reply: &O::Response) -> bool {
        Self::reply_encoded_len::<O>(reply) <= Self::LIMIT
    }

    pub fn encoded_len(&self) -> usize {
        match self {
            Self::Call { id, method, params } => call_encoded_len(*id, method, params.len()),
            Self::Answer { id, result } => answer_encoded_len(*id, result.len()),
            Self::Refusal { id, refusal } => refusal_encoded_len(*id, refusal),
            Self::Cancel { id } => 2 + varint_len(*id) + varint_len(u64::MAX),
            Self::Push { method, params } => {
                2usize.saturating_add(binary_bytes_len(method.len())).saturating_add(binary_bytes_len(params.len()))
            }
        }
    }

    pub fn fits(&self) -> bool {
        self.encoded_len() <= Self::LIMIT
    }

    #[cfg(feature = "kit")]
    pub(crate) fn exiting(reason: String) -> Self {
        Self::Push {
            method: Self::EXITING.to_owned(),
            params: reason.encode().into(),
        }
    }

    pub fn exiting_reason(params: &[u8]) -> Option<String> {
        String::decode(params).ok()
    }

    pub async fn diagnostic<R>(input: &mut R) -> std::io::Result<Option<String>>
    where
        R: AsyncBufRead + Unpin + Send,
    {
        loop {
            let mut bytes = Vec::new();
            let read = input.take(Self::LIMIT as u64 + 1).read_until(b'\n', &mut bytes).await?;
            if read == 0 {
                return Ok(None);
            }
            if bytes.len() > Self::LIMIT {
                let mut rest = Vec::new();
                input.read_until(b'\n', &mut rest).await?;
                continue;
            }

            let text = String::from_utf8_lossy(&bytes);
            return Ok(Some(text.trim_end_matches(['\n', '\r']).to_owned()));
        }
    }
}

// Postcard Length Of A Publication Without Its Following Rows Field.
#[cfg(feature = "kit")]
pub(crate) fn publication_encoded_len(
    stream: &str, schema: u32, sequence: u64, payload: usize, published_unix_micros: Option<i64>, context: Option<&str>,
) -> usize {
    binary_bytes_len(stream.len())
        .saturating_add(varint_len(u64::from(schema)))
        .saturating_add(1 + varint_len(sequence))
        .saturating_add(1 + published_unix_micros.map_or(0, zigzag_len))
        .saturating_add(1 + context.map_or(0, |value| binary_bytes_len(value.len())))
        .saturating_add(binary_bytes_len(payload))
}

// Postcard Length Of One Following Row.
#[cfg(feature = "kit")]
pub(crate) fn following_encoded_len(payload: usize, published_delta_micros: Option<i64>) -> usize {
    binary_bytes_len(payload).saturating_add(1 + published_delta_micros.map_or(0, zigzag_len))
}

pub(crate) fn varint_len(value: u64) -> usize {
    ((64 - value.leading_zeros()).max(1) as usize).div_ceil(7)
}

fn call_encoded_len(id: u64, method: &str, payload_len: usize) -> usize {
    2usize.saturating_add(varint_len(id)).saturating_add(binary_bytes_len(method.len())).saturating_add(binary_bytes_len(payload_len))
}

fn answer_encoded_len(id: u64, payload_len: usize) -> usize {
    2usize.saturating_add(varint_len(id)).saturating_add(binary_bytes_len(payload_len))
}

fn refusal_encoded_len(id: u64, refusal: &harmos_domain::Refusal) -> usize {
    let refusal = postcard::experimental::serialized_size(refusal).unwrap_or(usize::MAX);
    2usize.saturating_add(varint_len(id)).saturating_add(refusal)
}

#[cfg(feature = "kit")]
fn zigzag_len(value: i64) -> usize {
    varint_len(((value as u64) << 1) ^ ((value >> 63) as u64))
}

fn binary_bytes_len(length: usize) -> usize {
    varint_len(length as u64).saturating_add(length)
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmos_domain::MemorySize;

    #[test]
    fn unique_payload_compacts() {
        let mut bytes = Vec::with_capacity(128);
        bytes.extend(0..32_u8);
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();

        let payload = FramePayload::from_range(bytes, 8..24);
        let expected = std::mem::size_of::<FramePayload>() + std::mem::size_of::<Vec<u8>>() + 2 * std::mem::size_of::<usize>() + capacity;
        assert_eq!(payload.memory_size(), expected);

        let owned = payload.into_vec();
        assert_eq!(owned.as_ptr(), pointer);
        assert_eq!(owned.capacity(), capacity);
        assert_eq!(owned, (8..24).collect::<Vec<u8>>());
    }

    #[test]
    fn shared_payload_independent() {
        let mut bytes = Vec::with_capacity(128);
        bytes.extend(0..32_u8);
        let payload = FramePayload::from_range(bytes, 8..24);
        let alias = payload.clone();
        let pointer = alias.as_ptr();
        let memory = alias.memory_size();

        let mut owned = payload.into_vec();
        assert_ne!(owned.as_ptr(), pointer);
        owned[0] = 99;
        assert_eq!(alias.as_ptr(), pointer);
        assert_eq!(alias.as_slice(), (8..24).collect::<Vec<u8>>());
        assert_eq!(alias.memory_size(), memory);
    }

    #[test]
    fn request_sizes_match() {
        for size in [0, 127, 128, 16383, 16384, 2097151, 2097152] {
            let operation = "operation.測定";
            let prepare = WireCommand::Prepare {
                invocation: u64::MAX,
                operation: operation.into(),
                fingerprint: [255; 32],
                generation: u64::MAX,
                payload: vec![255; size],
            };
            let prepare = Frame::Call {
                id: u64::MAX,
                method: WIRE_METHOD.into(),
                params: prepare.encode().unwrap().into(),
            };
            let child = WireChildCall {
                context: [255; 32],
                generation_context: Some([255; 32]),
                values: Vec::new(),
                operation: operation.into(),
                fingerprint: [1; 32],
                generation: u64::MAX,
                payload: vec![255; size],
                submit: true,
            };
            let child = Frame::Call {
                id: u64::MAX,
                method: CHILD_METHOD.into(),
                params: child.encode().unwrap().into(),
            };
            assert_eq!(Frame::request_payload_encoded_len(operation, size), prepare.encoded_len().max(child.encoded_len()));
        }
        assert_eq!(Frame::request_payload_encoded_len("op", usize::MAX), usize::MAX);
    }

    #[test]
    fn typed_sizes_match() {
        use harmos_domain::model::storage::files::{ByteRange, FileCompletion, FileSource};
        use harmos_operations::storage::ReadFile;
        let request = ReadFile {
            source: FileSource::Local("data/測定.bin".into()),
            range: ByteRange { offset: 127, limit: Some(128) },
        };
        assert_eq!(
            Frame::request_encoded_len::<ReadFile>(&request),
            Frame::request_payload_encoded_len(ReadFile::ID, request.encode().len())
        );
        let reply = FileCompletion { bytes: u64::MAX };
        let envelope = WireAnswer::Ready { payload: reply.encode() }.encode().unwrap();
        let frame = Frame::Answer { id: u64::MAX, result: envelope.into() };
        assert_eq!(Frame::reply_encoded_len::<ReadFile>(&reply), frame.encoded_len());
        assert!(Frame::request_fits::<ReadFile>(&request));
        assert!(Frame::reply_fits::<ReadFile>(&reply));
    }

    #[test]
    fn child_growth_matches() {
        let values = vec![CallValue {
            key: "context.測定".into(),
            payload: vec![255; 16384],
        }];
        for generation in [false, true] {
            let empty = WireChildCall::payload_encoded_len("op", u64::MAX, 0, generation, &values).unwrap();
            for size in [0, 127, 128, 16383, 16384] {
                let call = WireChildCall {
                    context: [255; 32],
                    generation_context: generation.then_some([255; 32]),
                    values: values.clone(),
                    operation: "op".into(),
                    fingerprint: [1; 32],
                    generation: u64::MAX,
                    payload: vec![0; size],
                    submit: false,
                };
                let frame = Frame::Call {
                    id: u64::MAX,
                    method: CHILD_METHOD.into(),
                    params: call.encode().unwrap().into(),
                };
                assert_eq!(Frame::child_payload_encoded_len("op", u64::MAX, size, generation, &values), frame.encoded_len());
                assert_eq!(Frame::child_encoded_from_empty(empty, size), frame.encoded_len());
            }
        }
    }

    #[test]
    fn frame_prefix_limits() {
        let mut low = 0;
        let mut high = Frame::LIMIT + 1;
        while low + 1 < high {
            let middle = low + (high - low) / 2;
            if Frame::child_payload_encoded_len("op", u64::MAX, middle, true, &[]) <= Frame::LIMIT {
                low = middle;
            } else {
                high = middle;
            }
        }
        let mut call = WireChildCall {
            context: [0; 32],
            generation_context: Some([0; 32]),
            values: Vec::new(),
            operation: "op".into(),
            fingerprint: [1; 32],
            generation: u64::MAX,
            payload: vec![0; low],
            submit: true,
        };
        let frame = Frame::Call {
            id: u64::MAX,
            method: CHILD_METHOD.into(),
            params: call.encode().unwrap().into(),
        };
        assert_eq!(frame.encoded_len(), Frame::LIMIT);
        assert!(frame.fits());
        let packet = crate::transport::Packet::Frame(frame);
        assert_eq!(packet.encoded_len().unwrap(), Frame::LIMIT + 4);
        assert_eq!(packet.encode().unwrap().len(), Frame::LIMIT + 4);
        drop(packet);
        call.payload.push(0);
        let frame = Frame::Call {
            id: u64::MAX,
            method: CHILD_METHOD.into(),
            params: call.encode().unwrap().into(),
        };
        assert_eq!(frame.encoded_len(), Frame::LIMIT + 1);
        assert!(!frame.fits());
        assert!(crate::transport::Packet::Frame(frame).encoded_len().is_err());
    }
}
