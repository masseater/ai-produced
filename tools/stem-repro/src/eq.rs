use anyhow::Result;

use crate::features::{centers, ltas};
use crate::signal::{
    Frame, RATE, channel, fft_convolve, firwin2, interp, join, minimum_phase, mono,
};

pub fn curve(y: &[Frame], reference: &[Frame]) -> Result<Vec<f64>> {
    Ok(ltas(&mono(reference))?
        .iter()
        .zip(ltas(&mono(y))?)
        .map(|(r, c)| (r - c).clamp(-18.0, 18.0))
        .collect())
}

pub fn apply(y: &[Frame], gains: &[f64]) -> Result<Vec<Frame>> {
    let log_centers: Vec<f64> = centers().iter().map(|c| c.ln()).collect();
    let freq: Vec<f64> = (0..2049)
        .map(|i| f64::from(i) * RATE / 2.0 / 2048.0)
        .collect();
    let gain: Vec<f64> = freq
        .iter()
        .map(|f| 10f64.powf(interp(f.max(1.0).ln(), &log_centers, gains) / 20.0))
        .collect();
    let h = minimum_phase(&firwin2(4095, &freq, &gain)?)?;
    let mut filtered = Vec::with_capacity(2);
    for c in 0..2 {
        let mut out = fft_convolve(&channel(y, c), &h)?;
        out.truncate(y.len());
        filtered.push(out);
    }
    Ok(join(&filtered[0], &filtered[1]))
}

pub fn matched(y: &[Frame], reference: &[Frame], passes: usize) -> Result<(Vec<Frame>, Vec<f64>)> {
    let mut y = y.to_vec();
    let mut total = vec![0.0; centers().len()];
    for _ in 0..passes {
        let g = curve(&y, reference)?;
        y = apply(&y, &g)?;
        for (t, v) in total.iter_mut().zip(&g) {
            *t += v;
        }
    }
    Ok((y, total))
}
