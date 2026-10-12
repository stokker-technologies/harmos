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

// Local Imports
use super::*;
use crate::storage::QueryValue;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Transform<'a, T> {
    pub(super) series: &'a mut Series<T>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ReducedSeries {
    pub points: Vec<SeriesPoint>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Spectrum {
    pub frequency: Vec<f64>,
    pub real: Vec<f64>,
    pub imag: Vec<f64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<T: MemorySize + 'static> Transform<'_, T> {
    pub async fn downsample(
        self,
        count: NonZeroUsize,
        memory_bytes: NonZeroUsize,
    ) -> Result<SharedQueryValue<ReducedSeries>> {
        self.series.collect().await?;
        let view = self.series.view()?;
        let computation = view.begin_computation()?;
        let count = count.get().min(self.series.samples);
        let bytes =
            std::mem::size_of::<ReducedSeries>()
                .saturating_add(count.saturating_mul(
                    std::mem::size_of::<SeriesPoint>() + std::mem::size_of::<usize>(),
                ))
                .saturating_add(256);
        if bytes > memory_bytes.get() || memory_bytes > self.series.query.decoded_bytes() {
            return Err(invalid("downsample exceeds original output allowance"));
        }
        let reservation = view.manager().reserve_read_now(bytes, 1)?;
        let indices = crate::signal::decimate(self.series.samples, count);
        let mut points = slots(count)?;
        let mut selected = indices.iter().copied().peekable();
        for (index, point) in self.series.points().enumerate() {
            computation.check()?;
            if selected.peek() == Some(&index) {
                points.push(point.clone());
                selected.next();
            }
        }
        let value = QueryValue::retain(ReducedSeries { points }, reservation)?.try_into_shared()?;
        computation.finish(value)
    }
    #[cfg(feature = "fft")]
    pub async fn fft(
        self,
        rate: f64,
        taper: crate::signal::Taper,
        memory_bytes: NonZeroUsize,
    ) -> Result<SharedQueryValue<Spectrum>> {
        if !rate.is_finite() || rate <= 0.0 {
            return Err(invalid("FFT rate must be finite and positive"));
        }
        self.series.collect().await?;
        let length = self.series.samples;
        if !length.is_power_of_two() || length < 2 {
            return Err(invalid(
                "bounded FFT requires at least two power-of-two samples",
            ));
        }
        let view = self.series.view()?;
        let computation = view.begin_computation()?;
        let bins = length / 2 + 1;
        let bytes = length
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(bins.checked_mul(8)?))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Spectrum>() + 256))
            .ok_or(invalid("FFT memory overflow"))?;
        if bytes > memory_bytes.get()
            || memory_bytes > self.series.query.decoded_bytes()
            || bytes
                > view
                    .execution_limits()?
                    .ok_or(Error::ViewExpired)?
                    .memory_bytes
                    .get()
        {
            return Err(invalid("FFT exceeds original scratch/output allowance"));
        }
        let reservation = view.manager().reserve_read_now(bytes, 1)?;
        let mut real = slots(length)?;
        let mut imag = slots(length)?;
        let mut previous: Option<&ScalarValue> = None;
        let mut step = None;
        let mut gain = 0.0;
        for (index, point) in self.series.points().enumerate() {
            computation.check()?;
            if let Some(previous) = previous {
                let spacing = spacing(previous, &point.axis)?;
                if spacing <= 0.0
                    || step.is_some_and(|step: f64| (spacing - step).abs() > step.abs() * 1e-10)
                {
                    return Err(invalid("FFT requires uniformly increasing original axis"));
                }
                step = Some(spacing);
            }
            previous = Some(&point.axis);
            let coefficient = taper.coefficient(index, length);
            gain += coefficient;
            let value = numeric(&point.value)? * coefficient;
            if !value.is_finite() {
                return Err(invalid("FFT tapered value exceeds finite range"));
            }
            real.push(value);
            imag.push(0.0);
        }
        if !gain.is_finite() || gain <= 0.0 {
            return Err(invalid("FFT taper gain must be positive"));
        }
        let mut failure = None;
        let result =
            crate::signal::fft_in_place(&mut real, &mut imag, || match computation.check() {
                Ok(()) => true,
                Err(error) => {
                    failure = Some(error);
                    false
                }
            });
        if let Some(error) = failure {
            return Err(error);
        }
        result.map_err(|_| invalid("bounded FFT failed"))?;
        real.truncate(bins);
        imag.truncate(bins);
        let mut frequency = slots(bins)?;
        for index in 0..bins {
            computation.check()?;
            let factor = if index == 0 || index == length / 2 {
                1.0
            } else {
                2.0
            } / gain;
            real[index] *= factor;
            imag[index] *= factor;
            frequency.push(index as f64 * rate / length as f64);
            if !real[index].is_finite() || !imag[index].is_finite() || !frequency[index].is_finite()
            {
                return Err(invalid("FFT result exceeds finite range"));
            }
        }
        let value = QueryValue::retain(
            Spectrum {
                frequency,
                real,
                imag,
            },
            reservation,
        )?
        .try_into_shared()?;
        computation.finish(value)
    }
}
impl MemorySize for ReducedSeries {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(
            self.points
                .memory_size()
                .saturating_sub(std::mem::size_of::<Vec<SeriesPoint>>()),
        )
    }
}
impl MemorySize for Spectrum {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(
            self.frequency
                .capacity()
                .saturating_add(self.real.capacity())
                .saturating_add(self.imag.capacity())
                .saturating_mul(8),
        )
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn slots<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| invalid("transform allocation failed"))?;
    if values.capacity() != count {
        return Err(invalid("transform capacity exceeds reservation"));
    }
    Ok(values)
}
#[cfg(feature = "fft")]
fn numeric(value: &ScalarValue) -> Result<f64> {
    let converted = match *value {
        ScalarValue::Float(value) if value.is_finite() => value,
        ScalarValue::Unsigned(value) if (value as f64) as u128 == u128::from(value) => value as f64,
        ScalarValue::Signed(value) if (value as f64) as i128 == i128::from(value) => value as f64,
        ScalarValue::Exact(value)
            if value.denominator.get().is_power_of_two()
                && (value.numerator as f64) < 2f64.powi(128)
                && (value.numerator as f64) as u128 == value.numerator =>
        {
            let number = value.numerator as f64 / value.denominator.get() as f64;
            if value.negative { -number } else { number }
        }
        _ => return Err(invalid("FFT value cannot be represented exactly as f64")),
    };
    Ok(converted)
}
#[cfg(feature = "fft")]
fn spacing(a: &ScalarValue, b: &ScalarValue) -> Result<f64> {
    match (a, b) {
        (ScalarValue::Datetime(a), ScalarValue::Datetime(b)) => {
            let delta = b
                .checked_sub(*a)
                .ok_or(invalid("axis difference overflow"))?;
            if (delta as f64).abs() >= 2f64.powi(127) || (delta as f64) as i128 != delta {
                return Err(invalid("axis difference cannot be represented exactly"));
            }
            Ok(delta as f64)
        }
        (ScalarValue::Unsigned(a), ScalarValue::Unsigned(b)) => {
            let delta = i128::from(*b) - i128::from(*a);
            if (delta as f64).abs() >= 2f64.powi(127) || (delta as f64) as i128 != delta {
                return Err(invalid("axis difference cannot be represented exactly"));
            }
            Ok(delta as f64)
        }
        (ScalarValue::Signed(a), ScalarValue::Signed(b)) => {
            let delta = i128::from(*b) - i128::from(*a);
            if (delta as f64).abs() >= 2f64.powi(127) || (delta as f64) as i128 != delta {
                return Err(invalid("axis difference cannot be represented exactly"));
            }
            Ok(delta as f64)
        }
        (ScalarValue::Float(a), ScalarValue::Float(b)) => {
            let delta = b - a;
            if !delta.is_finite() {
                return Err(invalid("nonfinite axis difference"));
            }
            Ok(delta)
        }
        _ => Err(invalid("incompatible series axes")),
    }
}

/* === Tests === */
