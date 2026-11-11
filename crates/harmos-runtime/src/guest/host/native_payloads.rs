/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Convert the Closed Set of Deferred Guest Callees to Native Protobuf.

/* === Imports === */

use harmos_domain::operations::{Contract, WireDeclaration};
use harmos_domain::{Message, MessageError};
use harmos_operations::execution::operations::OperationFactory;
use prost::Message as _;
use std::sync::OnceLock;

/* === Enums === */

#[derive(Clone, Copy)]
pub(crate) enum Payload {
    HashFile,
    Publish,
}

/* === Implementations === */

impl Payload {
    fn contracts(self) -> &'static (Contract, Contract) {
        static PUBLISH: OnceLock<(Contract, Contract)> = OnceLock::new();
        static HASH: OnceLock<(Contract, Contract)> = OnceLock::new();
        match self {
            Self::Publish => PUBLISH.get_or_init(|| {
                (
                    WireDeclaration::of::<harmos_guest::builtins::streams::Publish>()
                        .contract()
                        .expect("private guest publication contract"),
                    OperationFactory::protobuf::<harmos_operations::streams::Publish>()
                        .expect("native publication declaration")
                        .contract()
                        .expect("native publication contract"),
                )
            }),
            Self::HashFile => HASH.get_or_init(|| {
                (
                    WireDeclaration::of::<harmos_guest::builtins::storage::HashFile>().contract().expect("private guest hash contract"),
                    OperationFactory::protobuf::<harmos_operations::storage::HashFile>()
                        .expect("native hash declaration")
                        .contract()
                        .expect("native hash contract"),
                )
            }),
        }
    }

    pub(crate) fn between(expected: &Contract, actual: &Contract) -> Option<Self> {
        [Self::HashFile, Self::Publish].into_iter().find(|payload| {
            let (guest, native) = payload.contracts();
            expected == guest && actual == native
        })
    }

    pub(crate) fn fingerprint(self) -> [u8; 32] {
        self.contracts().0.operation().fingerprint.0
    }

    pub(crate) fn preparation(self, bytes: &[u8]) -> Result<usize, MessageError> {
        let memory = match self {
            Self::HashFile => <harmos_guest::builtins::storage::HashFile as Message>::decoding_memory(bytes)?,
            Self::Publish => <harmos_guest::builtins::streams::Publish as Message>::decoding_memory(bytes)?,
        };
        memory.checked_mul(3).and_then(|n| n.checked_add(1024)).ok_or_else(|| MessageError::borrowed("guest conversion memory overflow"))
    }

    pub(crate) fn request(self, bytes: &[u8]) -> Result<Vec<u8>, MessageError> {
        match self {
            Self::Publish => {
                let request = <harmos_guest::builtins::streams::Publish as Message>::decode(bytes)?;
                Ok(harmos_protocol::host::Publish {
                    stream: request.stream,
                    sequence: request.sequence,
                    published_unix_micros: request.published_unix_micros,
                    context: request.context,
                    payload: request.payload,
                    following: request
                        .following
                        .into_iter()
                        .map(|row| harmos_protocol::host::FollowingRow {
                            payload: row.payload,
                            published_delta_micros: row.published_delta_micros,
                        })
                        .collect(),
                }
                .encode_to_vec())
            }
            Self::HashFile => {
                let request = <harmos_guest::builtins::storage::HashFile as Message>::decode(bytes)?;
                let request = harmos_protocol::storage::HashFile {
                    source: Some(request.source.try_into().map_err(|error: harmos_domain::Error| MessageError::new(error.to_string()))?),
                    range: Some(request.range.into()),
                };
                Ok(request.encode_to_vec())
            }
        }
    }

    pub(crate) fn response(self, bytes: &[u8]) -> Result<Vec<u8>, MessageError> {
        match self {
            Self::Publish => {
                harmos_protocol::host::Published::decode(bytes).map_err(|e| MessageError::new(e.to_string()))?;
                Ok(harmos_domain::Message::encode(&()))
            }
            Self::HashFile => {
                let response = harmos_protocol::storage::FileHash::decode(bytes).map_err(|e| MessageError::new(e.to_string()))?;
                let response = harmos_domain::model::storage::FileHash {
                    bytes_read: response.bytes_read,
                    sha256: response.sha256.try_into().map_err(|_| MessageError::borrowed("hash length differs"))?,
                };
                Ok(response.encode())
            }
        }
    }
}

pub(crate) fn matches(expected: &Contract, actual: &Contract) -> bool {
    expected == actual || Payload::between(expected, actual).is_some()
}
