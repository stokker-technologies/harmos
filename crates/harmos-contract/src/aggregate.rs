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

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
// The one statistic vocabulary every Harmos reduction answers: plot buckets,
// storage group-bys and stream batch measurements name what they compute with
// these members, and `harmos::signal` holds the math behind each of them.
// `Std` is the population standard deviation. `Percentile` carries a whole
// percentage, 0 to 100, answered by linear interpolation between ranks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aggregate {
    Count,
    Sum,
    Mean,
    Median,
    Min,
    Max,
    Rms,
    Std,
    Percentile(u8),
}

/* === Traits === */

/* === Implementations === */
impl Aggregate {
    // The wire name: the member in snake case, a percentile as `p` and its rank.
    pub fn name(&self) -> String {
        match self {
            Self::Count => "count".to_owned(),
            Self::Sum => "sum".to_owned(),
            Self::Mean => "mean".to_owned(),
            Self::Median => "median".to_owned(),
            Self::Min => "min".to_owned(),
            Self::Max => "max".to_owned(),
            Self::Rms => "rms".to_owned(),
            Self::Std => "std".to_owned(),
            Self::Percentile(rank) => format!("p{rank}"),
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        let named = match name {
            "count" => Self::Count,
            "sum" => Self::Sum,
            "mean" => Self::Mean,
            "median" => Self::Median,
            "min" => Self::Min,
            "max" => Self::Max,
            "rms" => Self::Rms,
            "std" => Self::Std,
            _ => {
                let rank = name.strip_prefix('p')?;
                if rank.is_empty() || !rank.bytes().all(|byte| byte.is_ascii_digit()) {
                    return None;
                }
                Self::Percentile(rank.parse().ok()?)
            }
        };

        named.answerable().then_some(named)
    }

    // A percentile rank is a percentage; every other member always answers.
    pub fn answerable(&self) -> bool {
        match self {
            Self::Percentile(rank) => *rank <= 100,
            _ => true,
        }
    }

    // Whether a running summary of count, sum, extremes and spread answers this
    // member, so partial answers merge without the values. Median and
    // percentiles need the ordered values themselves.
    pub fn mergeable(&self) -> bool {
        !matches!(self, Self::Median | Self::Percentile(_))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
