/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod connection;
mod consumption;
mod credit;
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
pub use connection::{Connection, Driver, Io, Received, Receiver, Sender};
pub use consumption::{Consumption, ReceiveCredit};
pub use credit::{Credit, Reservation};

/* === Imports === */
// Standard Library Imports
use std::io;
use std::sync::Arc;
// Third Party Imports
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
// Local Imports
use crate::Frame;

/* === Constants === */
const VERSION: u8 = 3;
const CONTROL_LIMIT: usize = 64;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Data,
    Control,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Packet {
    Frame(Frame),
    Credit(u64),
    Cancellation { id: u64, preceding: u64 },
}

// Variant Order Is Fixed By Version 3. No Owned Allocation Occurs While Decoding.
#[derive(Serialize, Deserialize)]
enum Body<'a> {
    Call {
        id: u64,
        method: &'a str,
        #[serde(borrow, with = "payload")]
        params: &'a [u8],
    },
    Cancel {
        id: u64,
        preceding: u64,
    },
    Push {
        method: &'a str,
        #[serde(borrow, with = "payload")]
        params: &'a [u8],
    },
    Answer {
        id: u64,
        #[serde(borrow, with = "payload")]
        result: &'a [u8],
    },
    Refusal {
        id: u64,
        refusal: std::borrow::Cow<'a, harmos_domain::Refusal>,
    },
    Credit {
        consumed: u64,
    },
}

/* === Traits === */

/* === Implementations === */
impl Role {
    fn limit(self) -> usize {
        match self {
            Self::Data => Frame::LIMIT,
            Self::Control => CONTROL_LIMIT,
        }
    }
}

impl Packet {
    pub fn channel(&self) -> Role {
        match self {
            Self::Frame(Frame::Cancel { .. }) | Self::Credit(_) | Self::Cancellation { .. } => {
                Role::Control
            }
            _ => Role::Data,
        }
    }

    pub fn encoded_len(&self) -> io::Result<usize> {
        let size =
            postcard::experimental::serialized_size(&(VERSION, self.body()?)).map_err(invalid)?;
        if size > self.channel().limit() {
            return Err(invalid("packet exceeds channel limit"));
        }
        Ok(size + 4)
    }

    pub fn encode(&self) -> io::Result<Vec<u8>> {
        let mut output = Vec::new();
        self.encode_into(&mut output)?;
        Ok(output)
    }

    fn encode_into(&self, output: &mut Vec<u8>) -> io::Result<()> {
        let size = self.encoded_len()?;
        if size > output.capacity() {
            output.reserve_exact(size - output.len());
        }
        output.resize(size, 0);
        output[..4].copy_from_slice(&((size - 4) as u32).to_be_bytes());
        postcard::to_slice(&(VERSION, self.body()?), &mut output[4..]).map_err(invalid)?;
        Ok(())
    }

    pub fn decode(channel: Role, bytes: &[u8]) -> io::Result<Self> {
        if bytes.is_empty() || bytes.len() > channel.limit() {
            return Err(invalid("packet exceeds channel limit or is empty"));
        }
        let (version, rest) = postcard::take_from_bytes::<u8>(bytes).map_err(invalid)?;
        if version != VERSION {
            return Err(invalid("unsupported sidecar transport version"));
        }
        let (body, rest) = postcard::take_from_bytes::<Body<'_>>(rest).map_err(invalid)?;
        if !rest.is_empty() {
            return Err(invalid("trailing sidecar transport bytes"));
        }
        let control = matches!(body, Body::Cancel { .. } | Body::Credit { .. });
        if control != (channel == Role::Control) {
            return Err(invalid("packet arrived on the wrong physical channel"));
        }
        Ok(match body {
            Body::Call { id, method, params } => Self::Frame(Frame::Call {
                id,
                method: method.to_owned(),
                params: params.to_vec(),
            }),
            Body::Cancel { id, preceding } => Self::Cancellation { id, preceding },
            Body::Push { method, params } => Self::Frame(Frame::Push {
                method: method.to_owned(),
                params: params.to_vec(),
            }),
            Body::Answer { id, result } => Self::Frame(Frame::Answer {
                id,
                result: result.to_vec(),
            }),
            Body::Refusal { id, refusal } => Self::Frame(Frame::Refusal {
                id,
                refusal: refusal.into_owned(),
            }),
            Body::Credit { consumed } => Self::Credit(consumed),
        })
    }

    fn body(&self) -> io::Result<Body<'_>> {
        Ok(match self {
            Self::Frame(Frame::Call { id, method, params }) => Body::Call {
                id: *id,
                method,
                params,
            },
            Self::Frame(Frame::Cancel { .. }) => {
                return Err(invalid("cancellation requires a data watermark"));
            }
            Self::Frame(Frame::Push { method, params }) => Body::Push { method, params },
            Self::Frame(Frame::Answer { id, result }) => Body::Answer { id: *id, result },
            Self::Frame(Frame::Refusal { id, refusal }) => Body::Refusal {
                id: *id,
                refusal: std::borrow::Cow::Borrowed(refusal),
            },
            Self::Credit(consumed) => Body::Credit {
                consumed: *consumed,
            },
            Self::Cancellation { id, preceding } => Body::Cancel {
                id: *id,
                preceding: *preceding,
            },
        })
    }
}

/* === Public Functions === */
pub async fn read<R: AsyncRead + Unpin>(
    reader: &mut R,
    channel: Role,
) -> io::Result<Option<Packet>> {
    let Some(length) = read_length(reader, channel).await? else {
        return Ok(None);
    };
    let mut body = vec![0; length];
    reader.read_exact(&mut body).await?;
    Packet::decode(channel, &body).map(Some)
}

pub async fn read_data<R: AsyncRead + Unpin>(
    reader: &mut R,
    credit: &Arc<ReceiveCredit>,
) -> io::Result<Option<(Packet, Consumption)>> {
    let Some(length) = read_length(reader, Role::Data).await? else {
        return Ok(None);
    };
    let consumption = credit.admit(length + 4)?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body).await?;
    let packet = Packet::decode(Role::Data, &body)?;
    Ok(Some((packet, consumption)))
}

pub async fn write<W: AsyncWrite + Unpin>(writer: &mut W, packet: &Packet) -> io::Result<()> {
    let bytes = packet.encode()?;
    writer.write_all(&bytes).await
}

/* === Private Functions === */
async fn read_length<R: AsyncRead + Unpin>(
    reader: &mut R,
    channel: Role,
) -> io::Result<Option<usize>> {
    let mut header = [0; 4];
    if reader.read(&mut header[..1]).await? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..]).await?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > channel.limit() {
        return Err(invalid("invalid sidecar packet length"));
    }
    Ok(Some(length))
}

fn invalid(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

/* === Tests === */
