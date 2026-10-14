/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};
// Local Imports
use harmos_contract::Message;
use harmos_contract::operations::{
    CHILD_METHOD, OperationDeclaration, WIRE_METHOD, WireAnswer, WireChildCall, WireCommand,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCatalog {
    pub registrations: Vec<crate::Registration>,
    pub observers: Vec<crate::inventory::observers::Declaration>,
    pub operations: Vec<crate::operations::Definition>,
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

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
    Call {
        id: u64,
        method: String,
        params: Vec<u8>,
    },
    Cancel {
        id: u64,
    },
    Push {
        method: String,
        params: Vec<u8>,
    },
    Answer {
        id: u64,
        result: Vec<u8>,
    },
    Refusal {
        id: u64,
        code: i64,
        message: String,
    },
}

/* === Traits === */

/* === Implementations === */
impl Frame {
    pub const REGISTER: &'static str = "harmos.register";
    pub const EXITING: &'static str = "harmos.exiting";
    pub const LIMIT: usize = 16 * 1024 * 1024;
    pub const OVERSIZED: i64 = -32_603;

    pub fn request_encoded_len<O: OperationDeclaration>(request: &O::Request) -> usize
    where
        O::Request: Message,
    {
        Self::request_payload_encoded_len(O::ID, request.encoded_len())
    }

    // The largest frame carrying an encoded request of `payload` bytes for
    // `operation`, as a routed call or as a child call.
    pub fn request_payload_encoded_len(operation: &str, payload: usize) -> usize {
        if operation.len() > Self::LIMIT {
            return usize::MAX;
        }
        let payload = binary_payload_growth(payload);
        let prepare = WireCommand::Prepare {
            invocation: u64::MAX,
            operation: operation.to_owned(),
            payload: Vec::new(),
        };
        let child = WireChildCall {
            context: [u8::MAX; 32],
            generation_context: Some([u8::MAX; 32]),
            values: Vec::new(),
            operation: operation.to_owned(),
            payload: Vec::new(),
            submit: true,
        };
        call_encoded_len(
            u64::MAX,
            WIRE_METHOD,
            prepare
                .encode()
                .map_or(usize::MAX, |bytes| bytes.len())
                .saturating_add(payload),
        )
        .max(call_encoded_len(
            u64::MAX,
            CHILD_METHOD,
            child
                .encode()
                .map_or(usize::MAX, |bytes| bytes.len())
                .saturating_add(payload),
        ))
    }

    pub fn request_fits<O: OperationDeclaration>(request: &O::Request) -> bool
    where
        O::Request: Message,
    {
        Self::request_encoded_len::<O>(request) <= Self::LIMIT
    }

    pub fn reply_encoded_len<O: OperationDeclaration>(reply: &O::Response) -> usize
    where
        O::Response: Message,
    {
        let empty = WireAnswer::Ready {
            payload: Vec::new(),
        };
        let payload = empty
            .encode()
            .map_or(usize::MAX, |bytes| bytes.len())
            .saturating_add(binary_payload_growth(reply.encoded_len()));
        answer_encoded_len(u64::MAX, payload)
    }

    pub fn reply_fits<O: OperationDeclaration>(reply: &O::Response) -> bool
    where
        O::Response: Message,
    {
        Self::reply_encoded_len::<O>(reply) <= Self::LIMIT
    }

    pub fn encoded_len(&self) -> usize {
        match self {
            Self::Call { id, method, params } => call_encoded_len(*id, method, params.len()),
            Self::Answer { id, result } => answer_encoded_len(*id, result.len()),
            Self::Refusal { id, code, message } => refusal_encoded_len(*id, *code, message.len()),
            Self::Cancel { id } => 2 + varint_len(*id) + varint_len(u64::MAX),
            Self::Push { method, params } => 2usize
                .saturating_add(binary_bytes_len(method.len()))
                .saturating_add(binary_bytes_len(params.len())),
        }
    }

    pub fn fits(&self) -> bool {
        self.encoded_len() <= Self::LIMIT
    }

    #[cfg(feature = "kit")]
    pub(crate) fn exiting(reason: String) -> Self {
        Self::Push {
            method: Self::EXITING.to_owned(),
            params: reason.encode_to_vec(),
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
            let read = input
                .take(Self::LIMIT as u64 + 1)
                .read_until(b'\n', &mut bytes)
                .await?;
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

/* === Public Functions === */
// Postcard Length Of A Publication Without Its Following Rows Field.
#[cfg(feature = "kit")]
pub(crate) fn publication_encoded_len(
    stream: &str,
    schema: u32,
    sequence: u64,
    payload: usize,
    published_unix_micros: Option<i64>,
    context: Option<&str>,
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

/* === Private Functions === */
fn binary_payload_growth(bytes: usize) -> usize {
    bytes.saturating_add(varint_len(bytes as u64).saturating_sub(1))
}

fn call_encoded_len(id: u64, method: &str, payload_len: usize) -> usize {
    2usize
        .saturating_add(varint_len(id))
        .saturating_add(binary_bytes_len(method.len()))
        .saturating_add(binary_bytes_len(payload_len))
}

fn answer_encoded_len(id: u64, payload_len: usize) -> usize {
    2usize
        .saturating_add(varint_len(id))
        .saturating_add(binary_bytes_len(payload_len))
}

fn refusal_encoded_len(id: u64, code: i64, message_len: usize) -> usize {
    let zigzag = ((code as u64) << 1) ^ ((code >> 63) as u64);
    2usize
        .saturating_add(varint_len(id))
        .saturating_add(varint_len(zigzag))
        .saturating_add(binary_bytes_len(message_len))
}

#[cfg(feature = "kit")]
fn zigzag_len(value: i64) -> usize {
    varint_len(((value as u64) << 1) ^ ((value >> 63) as u64))
}

fn binary_bytes_len(length: usize) -> usize {
    varint_len(length as u64).saturating_add(length)
}

/* === Tests === */
