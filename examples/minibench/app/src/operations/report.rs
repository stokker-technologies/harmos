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
use std::fmt;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use crate::resources::Resources;
use crate::streams::Reading;
use harmos::operations::Context;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq)]
pub struct Corrected {
    pub device: String,
    pub channel: u32,
    pub millivolts: i32,
}

pub struct Report {
    resources: Arc<Resources>,
}

pub struct ReportRequest {
    pub readings: Vec<Reading>,
    pub channel: u32,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq)]
pub enum ReportError {
    Unmeasured(u32),
}

/* === Traits === */

/* === Implementations === */
impl fmt::Display for ReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unmeasured(channel) => {
                write!(formatter, "no reading was captured for channel {channel}")
            }
        }
    }
}

impl std::error::Error for ReportError {}

#[harmos::operation(declaration = super::Report)]
impl Report {
    fn new(resources: Arc<Resources>) -> Result<Self, ReportError> {
        Ok(Self { resources })
    }

    async fn run(
        &mut self,
        _context: Context<'_>,
        request: &mut ReportRequest,
    ) -> Result<Corrected, ReportError> {
        let raw = request
            .readings
            .iter()
            .find(|reading| reading.channel == request.channel)
            .ok_or(ReportError::Unmeasured(request.channel))?;
        Ok(Corrected {
            device: self.resources.device.clone(),
            channel: request.channel,
            millivolts: self
                .resources
                .calibration
                .correct(request.channel, raw.millivolts),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
