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
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::protocol::Start;

/* === Constants === */
pub const MIB: usize = 1024 * 1024;
pub const WARMUP_MIB: u64 = 64;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy)]
pub struct Case {
    pub name: &'static str,
    pub samples: u32,
    pub payload_bytes: u32,
    pub pace_bytes_per_second: u64,
}

#[derive(Debug)]
pub struct Settings {
    pub sizes: Vec<u64>,
    pub rates: Vec<u64>,
    pub seconds: u64,
    pub repetitions: u64,
    pub burst_mib: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub useful_mib_s: f64,
    pub encoded_mib_s: f64,
    pub samples_s: f64,
    pub elapsed_s: f64,
    pub rejected: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Settings {
    pub fn load() -> Result<Self, String> {
        Self::parse(|name| match std::env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(error) => Err(format!("{name}: {error}")),
        })
    }

    pub fn parse(read: impl Fn(&str) -> Result<Option<String>, String>) -> Result<Self, String> {
        let list = |name, default: &str, max| -> Result<Vec<u64>, String> {
            let value = read(name)?.unwrap_or_else(|| default.to_owned());
            if value.split(',').count() > 8 {
                return Err(format!("{name}: at most 8 comma-separated values are allowed"));
            }
            let mut values = value.split(',').map(|value| number(name, value.trim(), max)).collect::<Result<Vec<_>, _>>()?;
            values.sort_unstable();
            if values.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(format!("{name}: duplicate values are not allowed"));
            }
            Ok(values)
        };
        let scalar = |name, default: &str, max| number(name, &read(name)?.unwrap_or_else(|| default.to_owned()), max);
        let settings = Self {
            sizes: list("HARMOS_BENCH_SIZES", "16384,65536", 1024 * 1024)?,
            rates: list("HARMOS_BENCH_RATES", "100,300,500,700", 4096)?,
            seconds: scalar("HARMOS_BENCH_SECONDS", "2", 30)?,
            repetitions: scalar("HARMOS_BENCH_REPETITIONS", "3", 10)?,
            burst_mib: scalar("HARMOS_BENCH_BURST_MIB", "256", 4096)?,
        };
        let cases = settings.sizes.len() as u64 * (settings.rates.len() as u64 + 1) * settings.repetitions;
        if cases > 256 {
            return Err("HARMOS_BENCH_SIZES/RATES/REPETITIONS: at most 256 recorded cases are allowed".into());
        }
        if settings.sizes.len() as u64 * settings.rates.len() as u64 * settings.repetitions * settings.seconds > 600 {
            return Err("HARMOS_BENCH_SECONDS/REPETITIONS: nominal sustained work exceeds 600 seconds".into());
        }
        let mut bytes = 0_u64;
        for &size in &settings.sizes {
            let warmup = settings.case(size, 0, WARMUP_MIB)?;
            bytes += u64::from(warmup.samples) * size;
            for rate in settings.targets() {
                let case = settings.case(size, rate, settings.burst_mib)?;
                bytes = bytes
                    .checked_add(u64::from(case.samples) * size * settings.repetitions)
                    .ok_or("HARMOS_BENCH settings overflow total transfer size")?;
            }
        }
        if bytes > 96 * 1024 * MIB as u64 {
            return Err("HARMOS_BENCH settings exceed the 96 GiB total useful transfer bound".into());
        }
        Ok(settings)
    }

    pub fn targets(&self) -> impl Iterator<Item = u64> + '_ {
        std::iter::once(0).chain(self.rates.iter().copied())
    }

    pub fn case(&self, size: u64, rate: u64, burst_mib: u64) -> Result<Case, String> {
        let bytes = if rate == 0 { burst_mib } else { rate * self.seconds } * MIB as u64;
        let samples = u32::try_from(bytes.div_ceil(size)).map_err(|_| "HARMOS_BENCH_SIZES/RATES/SECONDS: sample count exceeds u32")?;
        let start = Start { samples, payload_bytes: size as u32, pace_bytes_per_second: rate * MIB as u64, batch: 1 };
        start.validate().map_err(|error| format!("HARMOS_BENCH_SIZES/RATES/SECONDS/BURST_MIB: {error}"))?;
        Ok(Case { name: "capacity", samples, payload_bytes: start.payload_bytes, pace_bytes_per_second: start.pace_bytes_per_second })
    }

    pub fn timeout(&self, case: Case) -> Duration {
        let useful_bytes = u64::from(case.samples) * u64::from(case.payload_bytes);
        let nominal = self.seconds.max(useful_bytes.div_ceil(100 * MIB as u64));
        Duration::from_secs((nominal * 4 + 10).min(180))
    }
}

impl Observation {
    pub fn keeps_pace(self, target_mib_s: u64) -> bool {
        target_mib_s > 0 && self.useful_mib_s >= target_mib_s as f64 * 0.98 && self.rejected == 0
    }
}

/* === Public Functions === */
pub fn range(values: impl Iterator<Item = f64>) -> String {
    let mut values: Vec<_> = values.collect();
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    let median = if values.len().is_multiple_of(2) { (values[middle - 1] + values[middle]) / 2.0 } else { values[middle] };
    format!("{median:.3} [{:.3}, {:.3}]", values[0], values[values.len() - 1])
}

pub fn highest_met(cases: &[(u64, Vec<Observation>)]) -> Option<u64> {
    cases
        .iter()
        .filter(|(rate, trials)| !trials.is_empty() && trials.iter().all(|trial| trial.keeps_pace(*rate)))
        .map(|(rate, _)| *rate)
        .max()
}

/* === Private Functions === */
fn number(name: &str, value: &str, max: u64) -> Result<u64, String> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{name}: expected a positive integer of at most 20 decimal digits"));
    }
    let number = value.parse::<u64>().map_err(|_| format!("{name}: `{value}` must be a positive integer"))?;
    if !(1..=max).contains(&number) {
        return Err(format!("{name}: expected 1..={max}, received {number}"));
    }
    Ok(number)
}

/* === Tests === */
