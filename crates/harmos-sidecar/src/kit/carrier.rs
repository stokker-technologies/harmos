/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::io;

use crate::{Frame, transport};

pub(super) struct Arrival {
    pub frame: Frame,
    pub cancelled: bool,
    pub consumption: Option<transport::Consumption>,
}

pub(super) enum Failure {
    Invalid(String),
    Capacity,
    Panicked,
}

pub(super) enum Input {
    Binary(transport::Receiver),
}

pub(super) enum Sender {
    Binary(transport::Sender),
}

impl Input {
    pub async fn receive(&mut self) -> io::Result<Option<Arrival>> {
        let Self::Binary(input) = self;
        Ok(input.receive().await?.map(|received| Arrival {
            frame: received.frame,
            cancelled: received.cancelled,
            consumption: received.consumption,
        }))
    }
}

impl Sender {
    pub async fn send(&self, frame: Frame) -> io::Result<()> {
        let Self::Binary(output) = self;
        output.send(frame).await
    }

    pub fn fail(&self, failure: Failure) {
        let Self::Binary(output) = self;
        match failure {
            Failure::Invalid(message) => tracing::debug!(%message, "invalid sidecar frame"),
            Failure::Capacity => tracing::debug!("sidecar request queue exhausted"),
            Failure::Panicked => tracing::debug!("sidecar request handler panicked"),
        }
        output.close();
    }
}
