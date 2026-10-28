/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decibel {
    pub reference: f64,
    pub factor: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Scale {
    #[default]
    Linear,

    Decibel(Decibel),
    Ratio {
        factor: f64,
    },
}

impl Decibel {
    #[must_use]
    pub fn amplitude(reference: f64) -> Self {
        Self { reference, factor: 20.0 }
    }

    #[must_use]
    pub fn power(reference: f64) -> Self {
        Self { reference, factor: 10.0 }
    }

    #[must_use]
    pub fn volt() -> Self {
        Self::amplitude(1.0)
    }

    #[must_use]
    pub fn millivolt() -> Self {
        Self::amplitude(1e-3)
    }

    #[must_use]
    pub fn microvolt() -> Self {
        Self::amplitude(1e-6)
    }

    #[must_use]
    pub fn ampere() -> Self {
        Self::amplitude(1.0)
    }

    #[must_use]
    pub fn milliampere() -> Self {
        Self::amplitude(1e-3)
    }

    #[must_use]
    pub fn microampere() -> Self {
        Self::amplitude(1e-6)
    }

    #[must_use]
    pub fn sound_pressure() -> Self {
        Self::amplitude(20e-6)
    }

    #[must_use]
    pub fn watt() -> Self {
        Self::power(1.0)
    }

    #[must_use]
    pub fn milliwatt() -> Self {
        Self::power(1e-3)
    }

    #[must_use]
    pub fn to_db(&self, linear: f64) -> f64 {
        if linear > 0.0 { self.factor * (linear / self.reference).log10() } else { f64::NEG_INFINITY }
    }

    #[must_use]
    pub fn to_linear(&self, db: f64) -> f64 {
        self.reference * 10.0_f64.powf(db / self.factor)
    }

    #[must_use]
    pub fn is_compatible(&self, other: &Self) -> bool {
        (self.factor - other.factor).abs() < f64::EPSILON
    }

    #[must_use]
    pub fn offset_from(&self, other: &Self) -> Option<f64> {
        self.is_compatible(other).then(|| self.factor * (other.reference / self.reference).log10())
    }
}

impl Scale {
    #[must_use]
    pub fn factor(&self) -> Option<f64> {
        match self {
            Self::Linear => None,
            Self::Decibel(scale) => Some(scale.factor),
            Self::Ratio { factor } => Some(*factor),
        }
    }

    #[must_use]
    pub fn reference(&self) -> Option<f64> {
        match self {
            Self::Decibel(scale) => Some(scale.reference),
            Self::Linear | Self::Ratio { .. } => None,
        }
    }

    #[must_use]
    pub fn is_compatible(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Linear, Self::Linear) => true,
            (Self::Decibel(left), Self::Decibel(right)) => left.is_compatible(right),
            (Self::Decibel(scale), Self::Ratio { factor }) | (Self::Ratio { factor }, Self::Decibel(scale)) => {
                (scale.factor - factor).abs() < f64::EPSILON
            }
            (Self::Ratio { factor: left }, Self::Ratio { factor: right }) => (left - right).abs() < f64::EPSILON,
            (Self::Linear, _) | (_, Self::Linear) => false,
        }
    }

    #[must_use]
    pub fn sum(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (Self::Linear, Self::Linear) => Some(Self::Linear),
            (Self::Decibel(scale), Self::Ratio { .. }) | (Self::Ratio { .. }, Self::Decibel(scale)) if self.is_compatible(other) => {
                Some(Self::Decibel(*scale))
            }
            (Self::Ratio { factor }, Self::Ratio { .. }) if self.is_compatible(other) => Some(Self::Ratio { factor: *factor }),
            _ => None,
        }
    }

    #[must_use]
    pub fn difference(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (Self::Linear, Self::Linear) => Some(Self::Linear),
            (Self::Decibel(left), Self::Decibel(_)) if self.is_compatible(other) => Some(Self::Ratio { factor: left.factor }),
            (Self::Decibel(scale), Self::Ratio { .. }) if self.is_compatible(other) => Some(Self::Decibel(*scale)),
            (Self::Ratio { factor }, Self::Ratio { .. }) if self.is_compatible(other) => Some(Self::Ratio { factor: *factor }),
            _ => None,
        }
    }
}
