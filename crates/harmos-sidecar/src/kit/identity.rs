/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::{fmt, io};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActivationId([u8; 16]);

#[derive(Clone, Copy)]
pub(super) struct Identity {
    pub(super) observer: &'static str,
    pub(super) activation: ActivationId,
}

impl Identity {
    pub(super) fn new(observer: &'static str) -> io::Result<Self> {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes)?;
        Ok(Self {
            observer,
            activation: ActivationId(bytes),
        })
    }
}

impl fmt::Display for ActivationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}
