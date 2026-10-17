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
use std::f64::consts::PI;
// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taper {
    Rectangular,
    Hamming,
    Hann,
    Blackman,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Band {
    Rectangular {
        start: f64,
        end: f64,
    },
    RaisedCosine {
        start: f64,
        end: f64,
        transition: f64,
    },
    Kaiser {
        start: f64,
        end: f64,
        beta: f64,
    },
    Gaussian {
        start: f64,
        end: f64,
        sigma: f64,
    },
}

/* === Traits === */

/* === Implementations === */
impl Taper {
    #[must_use]
    pub fn coefficient(&self, index: usize, length: usize) -> f64 {
        let length = length as f64;
        let index = index as f64;

        match self {
            Self::Rectangular => 1.0,
            Self::Hamming => 0.54 - 0.46 * (2.0 * PI * index / length).cos(),
            Self::Hann => 0.5 * (1.0 - (2.0 * PI * index / length).cos()),
            Self::Blackman => {
                0.42 - 0.5 * (2.0 * PI * index / length).cos()
                    + 0.08 * (4.0 * PI * index / length).cos()
            }
        }
    }

    #[must_use]
    pub fn coefficients(&self, length: usize) -> Vec<f64> {
        (0..length)
            .map(|index| self.coefficient(index, length))
            .collect()
    }
}

impl Band {
    #[must_use]
    pub fn coefficient(&self, frequency: f64) -> f64 {
        let (start, end) = self.bounds();
        if frequency < start || frequency > end {
            return 0.0;
        }

        match self {
            Self::Rectangular { .. } => 1.0,
            Self::RaisedCosine { transition, .. } => {
                if frequency < start + transition {
                    0.5 * (1.0 - (PI * (frequency - start) / transition).cos())
                } else if frequency > end - transition {
                    0.5 * (1.0 - (PI * (end - frequency) / transition).cos())
                } else {
                    1.0
                }
            }
            Self::Kaiser { beta, .. } => {
                let half = (end - start) / 2.0;
                let offset = (frequency - (start + end) / 2.0) / half;
                bessel_i0(beta * (1.0 - offset * offset).sqrt()) / bessel_i0(*beta)
            }
            Self::Gaussian { sigma, .. } => {
                let offset = (frequency - (start + end) / 2.0) / sigma;
                (-0.5 * offset * offset).exp()
            }
        }
    }

    #[must_use]
    pub fn bounds(&self) -> (f64, f64) {
        match self {
            Self::Rectangular { start, end }
            | Self::RaisedCosine { start, end, .. }
            | Self::Kaiser { start, end, .. }
            | Self::Gaussian { start, end, .. } => (*start, *end),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn bessel_i0(argument: f64) -> f64 {
    let magnitude = argument.abs();

    if magnitude < 3.75 {
        let square = (argument / 3.75).powi(2);
        1.0 + square
            * (3.515_622_9
                + square
                    * (3.089_942_4
                        + square
                            * (1.206_749_2
                                + square
                                    * (0.265_973_2
                                        + square * (0.036_076_8 + square * 0.004_581_3)))))
    } else {
        let ratio = 3.75 / magnitude;
        (magnitude.exp() / magnitude.sqrt())
            * (0.398_942_28
                + ratio
                    * (0.013_285_92
                        + ratio
                            * (0.002_253_19
                                + ratio
                                    * (-0.001_575_65
                                        + ratio
                                            * (0.009_162_81
                                                + ratio
                                                    * (-0.020_577_06
                                                        + ratio
                                                            * (0.026_355_37
                                                                + ratio
                                                                    * (-0.016_476_33
                                                                        + ratio
                                                                            * 0.003_923_77))))))))
    }
}

/* === Tests === */
