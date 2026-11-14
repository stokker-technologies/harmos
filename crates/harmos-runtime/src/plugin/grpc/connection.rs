/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Single Physical gRPC Connection

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    future::Future,
    io,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

// Third Party Imports
use hyper_util::rt::TokioIo;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
    sync::watch,
};
use tonic::transport::{Channel, Endpoint};

// Local Imports
use super::Error;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Connection {
    pub(super) owner: String,
    pub(super) channel: Channel,
    message_bytes: usize,
    pub(super) closed: watch::Sender<bool>,
}

struct Socket {
    stream: TcpStream,
    closed: watch::Sender<bool>,
    closing: Pin<Box<dyn Future<Output = ()> + Send>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Connection {
    pub(super) fn client(&self) -> harmos_protocol::plugin::instance_client::InstanceClient<Channel> {
        harmos_protocol::plugin::instance_client::InstanceClient::new(self.channel.clone())
            .max_decoding_message_size(self.message_bytes)
            .max_encoding_message_size(self.message_bytes)
    }

    pub(super) async fn open(address: &str, message_bytes: usize) -> Result<Self, Error> {
        let url = if address.contains("://") { address.to_owned() } else { format!("http://{address}") };
        let endpoint = Endpoint::from_shared(url).map_err(|error| Error::Invalid(error.to_string()))?;
        let uri = endpoint.uri();
        if uri.scheme_str() != Some("http") || uri.authority().is_some_and(|authority| authority.as_str().contains('@')) {
            return Err(Error::Invalid("Only Plain gRPC Without Credentials Is Supported".into()));
        }
        let host = uri.host().ok_or_else(|| Error::Invalid("Missing Plugin Host".into()))?;
        let stream = TcpStream::connect((host, uri.port_u16().unwrap_or(80))).await?;
        stream.set_nodelay(true)?;
        let closed = watch::channel(false).0;
        let closing = Box::pin(super::disconnected(closed.subscribe()));
        let socket = Arc::new(Mutex::new(Some(Socket { stream, closed: closed.clone(), closing })));
        // A Channel Must Never Reconnect Behind An Activation's Handles.
        let connector = tower::service_fn(move |_| {
            let socket = socket.lock().expect("connection socket lock is healthy").take();
            async move { socket.map(TokioIo::new).ok_or_else(|| io::Error::new(io::ErrorKind::ConnectionAborted, "Plugin Connection Closed")) }
        });
        let channel = endpoint.connect_with_connector(connector).await.map_err(|error| Error::Transport(error.to_string()))?;
        let mut identity = [0; 16];
        getrandom::fill(&mut identity).map_err(|error| Error::Invalid(format!("Name Plugin Attachment: {error}")))?;
        let owner = format!("attachment_{}", u128::from_le_bytes(identity));
        Ok(Self { owner, channel, closed, message_bytes })
    }
}

impl AsyncRead for Socket {
    fn poll_read(mut self: Pin<&mut Self>, context: &mut Context<'_>, buffer: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        if self.closing.as_mut().poll(context).is_ready() {
            return Poll::Ready(Ok(()));
        }
        if buffer.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let before = buffer.filled().len();
        let result = Pin::new(&mut self.stream).poll_read(context, buffer);
        if matches!(&result, Poll::Ready(Err(_))) || matches!(&result, Poll::Ready(Ok(())) if buffer.filled().len() == before) {
            self.closed.send_replace(true);
        }
        result
    }
}

impl AsyncWrite for Socket {
    fn poll_write(mut self: Pin<&mut Self>, context: &mut Context<'_>, bytes: &[u8]) -> Poll<io::Result<usize>> {
        if *self.closed.borrow() {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "Plugin Connection Closed")));
        }
        let result = Pin::new(&mut self.stream).poll_write(context, bytes);
        if matches!(result, Poll::Ready(Err(_))) {
            self.closed.send_replace(true);
        }
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        let result = Pin::new(&mut self.stream).poll_flush(context);
        if matches!(result, Poll::Ready(Err(_))) {
            self.closed.send_replace(true);
        }
        result
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(context)
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        self.closed.send_replace(true);
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
