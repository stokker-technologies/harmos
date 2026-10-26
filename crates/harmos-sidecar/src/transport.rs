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
use std::io::{self, IoSlice};
use std::ops::Range;
use std::sync::Arc;
// Third Party Imports
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
// Local Imports
use crate::{Frame, FramePayload};

/* === Constants === */

const VERSION: u8 = 3;
const CONTROL_LIMIT: usize = 64;
const COALESCE_BYTES: usize = 64 * 1024;

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
            Self::Frame(Frame::Cancel { .. }) | Self::Credit(_) | Self::Cancellation { .. } => Role::Control,
            _ => Role::Data,
        }
    }

    pub fn encoded_len(&self) -> io::Result<usize> {
        let size = postcard::experimental::serialized_size(&(VERSION, self.body()?)).map_err(invalid)?;
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
            output.try_reserve_exact(size - output.len()).map_err(io::Error::other)?;
        }
        output.resize(size, 0);
        output[..4].copy_from_slice(&((size - 4) as u32).to_be_bytes());
        postcard::to_slice(&(VERSION, self.body()?), &mut output[4..]).map_err(invalid)?;
        Ok(())
    }

    #[tracing::instrument(target = "harmos::carrier", name = "sidecar.frame.decode", level = "trace", skip_all, fields(wire_bytes = bytes.len(), channel = ?channel))]
    pub fn decode(channel: Role, bytes: Vec<u8>) -> io::Result<Self> {
        if bytes.is_empty() || bytes.len() > channel.limit() {
            return Err(invalid("packet exceeds channel limit or is empty"));
        }
        let (version, rest) = postcard::take_from_bytes::<u8>(&bytes).map_err(invalid)?;
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
            Body::Call { id, method, params } => {
                let method = method.to_owned();
                let range = payload_range(&bytes, params)?;
                Self::Frame(Frame::Call { id, method, params: FramePayload::from_range(bytes, range) })
            }
            Body::Cancel { id, preceding } => Self::Cancellation { id, preceding },
            Body::Push { method, params } => {
                let method = method.to_owned();
                let range = payload_range(&bytes, params)?;
                Self::Frame(Frame::Push { method, params: FramePayload::from_range(bytes, range) })
            }
            Body::Answer { id, result } => {
                let range = payload_range(&bytes, result)?;
                Self::Frame(Frame::Answer { id, result: FramePayload::from_range(bytes, range) })
            }
            Body::Refusal { id, refusal } => Self::Frame(Frame::Refusal { id, refusal: refusal.into_owned() }),
            Body::Credit { consumed } => Self::Credit(consumed),
        })
    }

    fn body(&self) -> io::Result<Body<'_>> {
        Ok(match self {
            Self::Frame(Frame::Call { id, method, params }) => Body::Call { id: *id, method, params },
            Self::Frame(Frame::Cancel { .. }) => {
                return Err(invalid("cancellation requires a data watermark"));
            }
            Self::Frame(Frame::Push { method, params }) => Body::Push { method, params },
            Self::Frame(Frame::Answer { id, result }) => Body::Answer { id: *id, result },
            Self::Frame(Frame::Refusal { id, refusal }) => Body::Refusal { id: *id, refusal: std::borrow::Cow::Borrowed(refusal) },
            Self::Credit(consumed) => Body::Credit { consumed: *consumed },
            Self::Cancellation { id, preceding } => Body::Cancel { id: *id, preceding: *preceding },
        })
    }
}

/* === Public Functions === */

#[tracing::instrument(target = "harmos::carrier", name = "sidecar.frame.read", level = "trace", skip_all, fields(channel = ?channel))]
pub async fn read<R: AsyncRead + Unpin>(reader: &mut R, channel: Role) -> io::Result<Option<Packet>> {
    let Some(length) = read_length(reader, channel).await? else {
        return Ok(None);
    };
    let mut body = vec![0; length];
    reader.read_exact(&mut body).await?;
    Packet::decode(channel, body).map(Some)
}

#[tracing::instrument(target = "harmos::carrier", name = "sidecar.frame.read", level = "trace", skip_all, fields(channel = "Data"))]
pub async fn read_data<R: AsyncRead + Unpin>(reader: &mut R, credit: &Arc<ReceiveCredit>) -> io::Result<Option<(Packet, Consumption)>> {
    let Some(length) = read_length(reader, Role::Data).await? else {
        return Ok(None);
    };
    let consumption = credit.admit(length + 4)?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body).await?;
    let packet = Packet::decode(Role::Data, body)?;
    Ok(Some((packet, consumption)))
}

pub async fn write<W: AsyncWrite + Unpin>(writer: &mut W, packet: &Packet) -> io::Result<()> {
    write_with(writer, packet, &mut Vec::new()).await
}

/* === Private Functions === */

fn payload_range(bytes: &[u8], payload: &[u8]) -> io::Result<Range<usize>> {
    if payload.is_empty() {
        return Ok(0..0);
    }
    let start =
        (payload.as_ptr() as usize).checked_sub(bytes.as_ptr() as usize).ok_or_else(|| invalid("payload lies outside its packet"))?;
    let end = start.checked_add(payload.len()).filter(|end| *end <= bytes.len()).ok_or_else(|| invalid("payload exceeds its packet"))?;
    Ok(start..end)
}

#[tracing::instrument(target = "harmos::carrier", name = "sidecar.frame.write", level = "trace", skip_all, fields(wire_bytes = tracing::field::Empty, coalesced = tracing::field::Empty))]
async fn write_with<W: AsyncWrite + Unpin>(writer: &mut W, packet: &Packet, header: &mut Vec<u8>) -> io::Result<()> {
    let payload = match packet {
        Packet::Frame(Frame::Call { params, .. } | Frame::Push { params, .. }) => params.as_slice(),
        Packet::Frame(Frame::Answer { result, .. }) => result.as_slice(),
        _ => {
            packet.encode_into(header)?;
            tracing::Span::current().record("wire_bytes", header.len());
            return writer.write_all(header).await;
        }
    };
    let size = packet.encoded_len()?;
    let prefix_size = size - payload.len();
    let coalesce = !writer.is_write_vectored() && size <= COALESCE_BYTES;
    tracing::Span::current().record("wire_bytes", size);
    tracing::Span::current().record("coalesced", coalesce);
    let capacity = if coalesce { size } else { prefix_size };
    if capacity > header.capacity() {
        header.try_reserve_exact(capacity - header.len()).map_err(io::Error::other)?;
    }
    header.resize(prefix_size, 0);
    header[..4].copy_from_slice(&((size - 4) as u32).to_be_bytes());
    // Version 3's Fixed Body Tags Match The Canonical Serializer Above.
    match packet {
        Packet::Frame(Frame::Call { id, method, .. }) => {
            postcard::to_slice(&(VERSION, 0_u32, id, method.as_str(), payload.len()), &mut header[4..]).map_err(invalid)?;
        }
        Packet::Frame(Frame::Push { method, .. }) => {
            postcard::to_slice(&(VERSION, 2_u32, method.as_str(), payload.len()), &mut header[4..]).map_err(invalid)?;
        }
        Packet::Frame(Frame::Answer { id, .. }) => {
            postcard::to_slice(&(VERSION, 3_u32, id, payload.len()), &mut header[4..]).map_err(invalid)?;
        }
        _ => unreachable!("only payload frames use a split write"),
    }
    // Scalar Pipe Writers Otherwise Schedule Separate Header And Payload Writes.
    // Large Payloads Keep Their Original Allocation And The Split Write Path.
    if coalesce {
        header.extend_from_slice(payload);
        return writer.write_all(header).await;
    }
    let mut prefix = header.as_slice();
    let mut remaining = payload;
    while !prefix.is_empty() || !remaining.is_empty() {
        let slices = [IoSlice::new(prefix), IoSlice::new(remaining)];
        let written = writer.write_vectored(&slices).await?;
        if written == 0 {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "sidecar packet write made no progress"));
        }
        let prefix_bytes = written.min(prefix.len());
        prefix = &prefix[prefix_bytes..];
        remaining = &remaining[written - prefix_bytes..];
    }
    Ok(())
}

async fn read_length<R: AsyncRead + Unpin>(reader: &mut R, channel: Role) -> io::Result<Option<usize>> {
    let mut header = [0; 4];
    let read = reader.read(&mut header).await?;
    if read == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[read..]).await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    #[derive(Default)]
    struct ScalarWriter {
        bytes: Vec<u8>,
        writes: usize,
        maximum: Option<usize>,
    }

    struct PrefixReader {
        bytes: Vec<u8>,
        position: usize,
        maximum: usize,
        reads: usize,
        interrupted: bool,
    }

    impl PrefixReader {
        fn new(bytes: Vec<u8>, maximum: usize) -> Self {
            Self { bytes, position: 0, maximum, reads: 0, interrupted: false }
        }
    }

    impl AsyncRead for PrefixReader {
        fn poll_read(mut self: Pin<&mut Self>, _: &mut Context<'_>, output: &mut tokio::io::ReadBuf<'_>) -> Poll<io::Result<()>> {
            self.reads += 1;
            if std::mem::take(&mut self.interrupted) {
                let error = io::ErrorKind::Interrupted.into();
                return Poll::Ready(Err(error));
            }
            let remaining = self.bytes.len() - self.position;
            let count = remaining.min(self.maximum).min(output.remaining());
            let end = self.position + count;
            output.put_slice(&self.bytes[self.position..end]);
            self.position = end;
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for ScalarWriter {
        fn poll_write(mut self: Pin<&mut Self>, _: &mut Context<'_>, bytes: &[u8]) -> Poll<io::Result<usize>> {
            let count = bytes.len().min(self.maximum.unwrap_or(usize::MAX));
            self.bytes.extend_from_slice(&bytes[..count]);
            self.writes += 1;
            Poll::Ready(Ok(count))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn prefix_reads_once() {
        let mut reader = PrefixReader::new(128_u32.to_be_bytes().to_vec(), 4);
        assert_eq!(read_length(&mut reader, Role::Data).await.unwrap(), Some(128));
        assert_eq!(reader.reads, 1);

        for maximum in 1..4 {
            let mut reader = PrefixReader::new(128_u32.to_be_bytes().to_vec(), maximum);
            assert_eq!(read_length(&mut reader, Role::Data).await.unwrap(), Some(128));
            assert_eq!(reader.reads, 4_usize.div_ceil(maximum));
        }
    }

    #[tokio::test]
    async fn prefix_eof_errors() {
        let mut reader = PrefixReader::new(Vec::new(), 4);
        assert_eq!(read_length(&mut reader, Role::Data).await.unwrap(), None);
        for length in 1..4 {
            let bytes = 128_u32.to_be_bytes()[..length].to_vec();
            let mut reader = PrefixReader::new(bytes, 4);
            let error = read_length(&mut reader, Role::Data).await.unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        }
    }

    #[tokio::test]
    async fn prefix_invalid_interrupted() {
        for (channel, length) in [(Role::Data, 0), (Role::Data, Frame::LIMIT + 1), (Role::Control, 0), (Role::Control, CONTROL_LIMIT + 1)] {
            let bytes = u32::try_from(length).unwrap().to_be_bytes().to_vec();
            let mut reader = PrefixReader::new(bytes, 4);
            let error = read_length(&mut reader, channel).await.unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }

        let mut reader = PrefixReader::new(128_u32.to_be_bytes().to_vec(), 4);
        reader.interrupted = true;
        let error = read_length(&mut reader, Role::Data).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(reader.position, 0);
        assert_eq!(read_length(&mut reader, Role::Data).await.unwrap(), Some(128));
    }

    #[tokio::test]
    async fn scalar_frames_coalesce() {
        let mut scratch = Vec::new();
        for frame in [
            Frame::Call { id: 1, method: "call".into(), params: vec![23; 4096].into() },
            Frame::Push { method: "push".into(), params: vec![23; 8192].into() },
            Frame::Answer { id: 2, result: vec![23; 32_768].into() },
        ] {
            let packet = Packet::Frame(frame);
            let expected = packet.encode().unwrap();
            let mut writer = ScalarWriter::default();
            write_with(&mut writer, &packet, &mut scratch).await.unwrap();
            assert_eq!(writer.bytes, expected);
            assert_eq!(writer.writes, 1);
            assert_eq!(scratch.capacity(), expected.len());
        }
    }

    #[tokio::test]
    async fn scalar_partial_bulk() {
        let packet = Packet::Frame(Frame::Answer { id: 1, result: vec![7; 4096].into() });
        let mut writer = ScalarWriter { maximum: Some(3), ..ScalarWriter::default() };
        write_with(&mut writer, &packet, &mut Vec::new()).await.unwrap();
        assert_eq!(writer.bytes, packet.encode().unwrap());

        let packet = Packet::Frame(Frame::Answer { id: 2, result: vec![7; COALESCE_BYTES].into() });
        let mut writer = ScalarWriter::default();
        let mut scratch = Vec::new();
        write_with(&mut writer, &packet, &mut scratch).await.unwrap();
        assert_eq!(writer.bytes, packet.encode().unwrap());
        assert_eq!(writer.writes, 2);
        assert!(scratch.capacity() < 32);
    }

    #[test]
    fn malformed_payloads() {
        let bytes = vec![1, 2, 3];
        let foreign = vec![4, 5, 6];
        assert!(payload_range(&bytes, &foreign).is_err());
        assert!(std::panic::catch_unwind(|| FramePayload::from_range(vec![1], 0..2)).is_err());

        let packet = Packet::Frame(Frame::Answer { id: 1, result: vec![7; 128].into() });
        let encoded = packet.encode().unwrap();
        let mut body = encoded[4..].to_vec();
        body.pop();
        assert!(Packet::decode(Role::Data, body).is_err());
        let mut body = encoded[4..].to_vec();
        body.push(0);
        assert!(Packet::decode(Role::Data, body).is_err());
    }

    #[test]
    fn payload_keeps_backing() {
        for frame in [
            Frame::Call { id: 1, method: "call".into(), params: vec![23; 4096].into() },
            Frame::Push { method: "push".into(), params: vec![23; 4096].into() },
            Frame::Answer { id: 2, result: vec![23; 4096].into() },
        ] {
            let encoded = Packet::Frame(frame).encode().unwrap();
            let body = encoded[4..].to_vec();
            let expected = body.as_ptr().wrapping_add(body.len() - 4096);
            let Packet::Frame(frame) = Packet::decode(Role::Data, body).unwrap() else { panic!("payload frame") };
            let payload = match frame {
                Frame::Call { params, .. } | Frame::Push { params, .. } => params,
                Frame::Answer { result, .. } => result,
                _ => unreachable!(),
            };
            assert_eq!(payload.as_ptr(), expected);
            let retained = payload.clone();
            drop(payload);
            assert_eq!(retained.as_ptr(), expected);
            assert_eq!(retained.as_slice(), [23; 4096]);
        }
    }
}
