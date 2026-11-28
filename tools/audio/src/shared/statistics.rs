use anyhow::{Context, Result};
use num_traits::ToPrimitive;

#[must_use]
pub fn round(value: f64, digits: i32) -> f64 {
    let scale = 10f64.powi(digits);
    (value * scale).round_ties_even() / scale
}

pub fn percentile(values: &[f64], q: f64) -> Result<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let last = sorted
        .len()
        .checked_sub(1)
        .context("percentile of no values")?;
    let position = q / 100.0 * last.to_f64().context("value count")?;
    let floor = position.floor();
    let low = floor
        .to_usize()
        .filter(|low| *low <= last)
        .with_context(|| format!("bad percentile {q}"))?;
    let high = (low + 1).min(last);
    Ok(sorted[low] + (sorted[high] - sorted[low]) * (position - floor))
}

pub fn mean(values: impl IntoIterator<Item = f64>) -> Result<f64> {
    let (sum, count) = values
        .into_iter()
        .fold((0.0, 0usize), |(sum, count), v| (sum + v, count + 1));
    Ok(sum / count.to_f64().context("value count")?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_interpolates_like_numpy() {
        let values = [1.0, 2.0, 3.0, 4.0, 10.0];
        assert!((percentile(&values, 90.0).unwrap() - 7.6).abs() < 1e-12);
        assert!((percentile(&values, 10.0).unwrap() - 1.4).abs() < 1e-12);
    }

    #[test]
    fn rounds_to_digits() {
        assert!((round(1.23456, 2) - 1.23).abs() < 1e-12);
        assert!((round(-6.66, 1) + 6.7).abs() < 1e-12);
    }
}
