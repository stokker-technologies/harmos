/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::future::Future;

use tokio::sync::{oneshot, watch};

use super::carrier::{Input, Sender};
use super::{SharedOpening, exchange};

pub(super) async fn run_binary(
    opening: SharedOpening, data: crate::transport::Io, control: crate::transport::Io, stop: impl Future<Output = ()>,
) -> std::io::Result<()> {
    run_binary_with_clock(opening, data, control, stop, super::host::published_at).await
}

pub(super) async fn run_binary_with_clock(
    opening: SharedOpening, data: crate::transport::Io, control: crate::transport::Io, stop: impl Future<Output = ()>,
    clock: fn() -> Option<i64>,
) -> std::io::Result<()> {
    let crate::transport::Connection { sender, receiver, mut driver } = crate::transport::Connection::new(data, control);
    let (completed, _completion) = oneshot::channel();
    let (closing, stopping) = watch::channel(false);
    let mut input = Input::Binary(receiver);
    let output = Sender::Binary(sender.clone());
    let mut serving = Box::pin(exchange(opening, &mut input, &output, completed, stopping, clock));
    let mut stop = Box::pin(stop);
    tokio::select! {
        result = &mut serving => {
            let flushed = tokio::select! {
                result = sender.drain() => result,
                result = driver.run() => result,
                _ = &mut stop => Ok(()),
            };
            sender.close();
            let ended = driver.run().await;
            result.map_err(std::io::Error::other)?;
            flushed?;
            ended
        },
        result = driver.run() => {
            if let Err(error) = result { tracing::debug!(%error, "binary sidecar transport ended"); }
            closing.send_replace(true);
            serving.await.map_err(std::io::Error::other)
        },
        _ = &mut stop => {
            sender.close();
            closing.send_replace(true);
            let settled = serving.await;
            let _ = driver.run().await;
            settled.map_err(std::io::Error::other)
        }
    }
}
