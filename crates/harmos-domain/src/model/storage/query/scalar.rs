/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::cmp::Ordering;
use std::num::NonZeroU128;

use serde::{Deserialize, Serialize};

use crate::MessageError;
use crate::model::storage::codec::{
    Flag, Float, Nested, Percent, Signed as SignedInt, Text, Unsigned, Wide, WideNonZero, WideSigned, coded, product, variants,
};
use crate::{Aggregate, Error, MemorySize, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactNumber {
    pub negative: bool,
    pub numerator: u128,
    pub denominator: NonZeroU128,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScalarValue {
    Missing,
    Null,
    Bool(bool),
    Signed(i64),
    Unsigned(u64),
    Float(f64),
    Text(String),
    Datetime(i128),
    Exact(ExactNumber),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalarKind {
    Bool,
    Signed,
    Unsigned,
    Float,
    Text,
    Datetime,
    Exact,
}
// What a storage group-by reduces a column to. `Len` and `Last` are row
// selections rather than statistics: `Len` counts rows including missing and
// null values, and `Last` keeps the value captured latest, so they stay here.
// Every statistic is a member of the contract `Aggregate` vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Agg {
    Len,
    Last,
    Of(Aggregate),
}

impl ExactNumber {
    pub fn new(negative: bool, numerator: u128, denominator: NonZeroU128) -> Self {
        let factor = gcd(numerator, denominator.get());
        Self {
            negative: negative && numerator != 0,
            numerator: numerator / factor,
            denominator: NonZeroU128::new(denominator.get() / factor).unwrap(),
        }
    }

    pub fn checked_add(self, other: Self) -> Result<Self> {
        let common = gcd(self.denominator.get(), other.denominator.get());
        let a = other.denominator.get() / common;
        let b = self.denominator.get() / common;
        let left = self.numerator.checked_mul(a).ok_or(invalid("exact arithmetic overflow"))?;
        let right = other.numerator.checked_mul(b).ok_or(invalid("exact arithmetic overflow"))?;
        let (negative, numerator) = if self.negative == other.negative {
            (self.negative, left.checked_add(right).ok_or(invalid("exact arithmetic overflow"))?)
        } else if left >= right {
            (self.negative, left - right)
        } else {
            (other.negative, right - left)
        };
        let denominator = self.denominator.get().checked_mul(a).ok_or(invalid("exact arithmetic overflow"))?;
        Ok(Self::new(negative, numerator, NonZeroU128::new(denominator).unwrap()))
    }

    pub fn divide(self, count: u128) -> Result<Self> {
        let factor = gcd(self.numerator, count);
        let denominator = self.denominator.get().checked_mul(count / factor).ok_or(invalid("exact arithmetic overflow"))?;
        Ok(Self::new(self.negative, self.numerator / factor, NonZeroU128::new(denominator).unwrap()))
    }

    fn compare(self, other: Self) -> Ordering {
        match (self.negative, other.negative) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => {
                let order = ratio_cmp(self.numerator, self.denominator.get(), other.numerator, other.denominator.get());
                if self.negative { order.reverse() } else { order }
            }
        }
    }
}
impl ScalarValue {
    pub fn check(&self) -> Result<()> {
        if matches!(self, Self::Float(value) if !value.is_finite()) {
            return Err(invalid("nonfinite scalar"));
        }
        if let Self::Exact(value) = self
            && ExactNumber::new(value.negative, value.numerator, value.denominator) != *value
        {
            return Err(invalid("noncanonical exact number"));
        }
        Ok(())
    }

    pub fn kind(&self) -> Option<ScalarKind> {
        Some(match self {
            Self::Missing | Self::Null => return None,
            Self::Bool(_) => ScalarKind::Bool,
            Self::Signed(_) => ScalarKind::Signed,
            Self::Unsigned(_) => ScalarKind::Unsigned,
            Self::Float(_) => ScalarKind::Float,
            Self::Text(_) => ScalarKind::Text,
            Self::Datetime(_) => ScalarKind::Datetime,
            Self::Exact(_) => ScalarKind::Exact,
        })
    }

    pub fn compare(&self, other: &Self) -> Result<Ordering> {
        self.check()?;
        other.check()?;
        if matches!(self, Self::Missing) {
            return Ok(if matches!(other, Self::Missing) { Ordering::Equal } else { Ordering::Less });
        }
        if matches!(other, Self::Missing) {
            return Ok(Ordering::Greater);
        }
        if matches!(self, Self::Null) {
            return Ok(if matches!(other, Self::Null) { Ordering::Equal } else { Ordering::Less });
        }
        if matches!(other, Self::Null) {
            return Ok(Ordering::Greater);
        }
        Ok(match (self, other) {
            (Self::Bool(a), Self::Bool(b)) => a.cmp(b),
            (Self::Signed(a), Self::Signed(b)) => a.cmp(b),
            (Self::Unsigned(a), Self::Unsigned(b)) => a.cmp(b),
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(b).unwrap(),
            (Self::Text(a), Self::Text(b)) => a.cmp(b),
            (Self::Datetime(a), Self::Datetime(b)) => a.cmp(b),
            (a, b) if exact(a).is_some() && exact(b).is_some() => exact(a).unwrap().compare(exact(b).unwrap()),
            _ => return Err(invalid("incompatible scalar comparison")),
        })
    }
}
impl MemorySize for ScalarValue {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(match self {
            Self::Text(text) => text.capacity(),
            _ => 0,
        })
    }
}

impl From<Aggregate> for Agg {
    fn from(kind: Aggregate) -> Self {
        Self::Of(kind)
    }
}

impl MemorySize for Agg {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl MemorySize for ExactNumber {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

// Only The Reduced Form Of An Exact Fraction Is Accepted On Decode.
product!(
    ExactNumber {
        negative: Flag => 1,
        numerator: Wide => 2,
        denominator: WideNonZero => 3,
    },
    check = |value: &ExactNumber| {
        if ExactNumber::new(value.negative, value.numerator, value.denominator) == *value {
            Ok(())
        } else {
            Err(MessageError::borrowed("exact number is not reduced"))
        }
    }
);

coded!(ScalarKind {
    Bool,
    Signed,
    Unsigned,
    Float,
    Text,
    Datetime,
    Exact
});
variants!(Agg {
    0, 1, "len": Self::Len => {} => Self::Len;
    1, 2, "last": Self::Last => {} => Self::Last;
    2, 3, "count": Self::Of(Aggregate::Count) => {} => Self::Of(Aggregate::Count);
    3, 4, "sum": Self::Of(Aggregate::Sum) => {} => Self::Of(Aggregate::Sum);
    4, 5, "mean": Self::Of(Aggregate::Mean) => {} => Self::Of(Aggregate::Mean);
    5, 6, "median": Self::Of(Aggregate::Median) => {} => Self::Of(Aggregate::Median);
    6, 7, "min": Self::Of(Aggregate::Min) => {} => Self::Of(Aggregate::Min);
    7, 8, "max": Self::Of(Aggregate::Max) => {} => Self::Of(Aggregate::Max);
    8, 9, "rms": Self::Of(Aggregate::Rms) => {} => Self::Of(Aggregate::Rms);
    9, 10, "std": Self::Of(Aggregate::Std) => {} => Self::Of(Aggregate::Std);
    10, 11, "percentile": Self::Of(Aggregate::Percentile(rank)) => { rank: Percent => 1 } => Self::Of(Aggregate::Percentile(rank));
});

variants!(ScalarValue {
    0, 1, "missing": Self::Missing => {} => Self::Missing;
    1, 2, "null": Self::Null => {} => Self::Null;
    2, 3, "bool": Self::Bool(value) => { value: Flag => 1 } => Self::Bool(value);
    3, 4, "signed": Self::Signed(value) => { value: SignedInt => 1 } => Self::Signed(value);
    4, 5, "unsigned": Self::Unsigned(value) => { value: Unsigned => 1 } => Self::Unsigned(value);
    5, 6, "float": Self::Float(value) => { value: Float => 1 } => Self::Float(value);
    6, 7, "text": Self::Text(value) => { value: Text => 1 } => Self::Text(value);
    7, 8, "datetime": Self::Datetime(value) => { value: WideSigned => 1 } => Self::Datetime(value);
    8, 9, "exact": Self::Exact(value) => { value: Nested<ExactNumber> => 1 } => Self::Exact(value);
});

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage reduce", reason }
}
fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
pub fn exact(value: &ScalarValue) -> Option<ExactNumber> {
    Some(match value {
        ScalarValue::Signed(value) => ExactNumber::new(*value < 0, u128::from(value.unsigned_abs()), NonZeroU128::MIN),
        ScalarValue::Unsigned(value) => ExactNumber::new(false, u128::from(*value), NonZeroU128::MIN),
        ScalarValue::Exact(value) => *value,
        _ => return None,
    })
}
// Continued-Fraction Comparison Avoids Cross-Product Overflow.
pub fn ratio_cmp(mut a: u128, mut b: u128, mut c: u128, mut d: u128) -> Ordering {
    let mut reverse = false;
    loop {
        let order = (a / b).cmp(&(c / d));
        if !order.is_eq() {
            return if reverse { order.reverse() } else { order };
        }
        let (r, s) = (a % b, c % d);
        if r == 0 || s == 0 {
            let order = r.cmp(&s);
            return if reverse { order.reverse() } else { order };
        }
        (a, b, c, d) = (b, r, d, s);
        reverse = !reverse;
    }
}
