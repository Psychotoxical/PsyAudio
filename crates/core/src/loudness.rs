//! EBU R128 loudness (used by ReplayGain) and peak measurement.

use crate::audio::Audio;

#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 3],
}

fn k_weighting(rate: f64) -> (Biquad, Biquad) {
    use std::f64::consts::PI;
    let f0 = 1681.974450955533;
    let g = 3.999843853973347;
    let q = 0.7071752369554196;
    let k = (PI * f0 / rate).tan();
    let vh = 10f64.powf(g / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / q + k * k;
    let pre = Biquad {
        b: [(vh + vb * k / q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0],
        a: [1.0, 2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
    };
    let f0 = 38.13547087602444;
    let q = 0.5003270373238773;
    let k = (PI * f0 / rate).tan();
    let d = 1.0 + k / q + k * k;
    let rlb = Biquad { b: [1.0, -2.0, 1.0], a: [1.0, 2.0 * (k * k - 1.0) / d, (1.0 - k / q + k * k) / d] };
    (pre, rlb)
}

/// Channel weights after ITU-R BS.1770: surrounds 1.41, the LFE of 5.1 is not counted.
fn channel_weight(ch: usize, channels: usize) -> f64 {
    match channels {
        5 if ch >= 3 => 1.41,
        6 if ch == 3 => 0.0,
        6 if ch >= 4 => 1.41,
        _ => 1.0,
    }
}

/// Per-block (400 ms, 75 % overlap) weighted mean-square energies of a track.
pub fn block_energies(a: &Audio) -> Vec<f64> {
    let ch = a.channels.max(1);
    let frames = a.frames();
    if frames == 0 || a.rate == 0 {
        return Vec::new();
    }
    let (pre, rlb) = k_weighting(a.rate as f64);
    // filtered squared samples summed over channels with weights
    let mut weighted = vec![0f64; frames];
    for c in 0..ch {
        let w = channel_weight(c, ch);
        if w == 0.0 {
            continue;
        }
        let (mut x1, mut x2, mut y1, mut y2) = (0f64, 0f64, 0f64, 0f64);
        let (mut u1, mut u2, mut z1, mut z2) = (0f64, 0f64, 0f64, 0f64);
        for i in 0..frames {
            let x = a.data[i * ch + c] as f64;
            let y = pre.b[0] * x + pre.b[1] * x1 + pre.b[2] * x2 - pre.a[1] * y1 - pre.a[2] * y2;
            x2 = x1;
            x1 = x;
            y2 = y1;
            y1 = y;
            let z = rlb.b[0] * y + rlb.b[1] * u1 + rlb.b[2] * u2 - rlb.a[1] * z1 - rlb.a[2] * z2;
            u2 = u1;
            u1 = y;
            z2 = z1;
            z1 = z;
            weighted[i] += w * z * z;
        }
    }
    let block = (a.rate as f64 * 0.4) as usize;
    let hop = (a.rate as f64 * 0.1) as usize;
    let mut prefix = vec![0f64; frames + 1];
    for i in 0..frames {
        prefix[i + 1] = prefix[i] + weighted[i];
    }
    let mut out = Vec::new();
    if frames < block {
        out.push(prefix[frames] / frames as f64);
        return out;
    }
    let mut start = 0;
    while start + block <= frames {
        out.push((prefix[start + block] - prefix[start]) / block as f64);
        start += hop;
    }
    out
}

/// Gated integrated loudness in LUFS of a set of block energies.
pub fn integrated(blocks: &[f64]) -> f64 {
    let lufs = |e: f64| -0.691 + 10.0 * e.max(1e-20).log10();
    let abs: Vec<f64> = blocks.iter().copied().filter(|&e| lufs(e) > -70.0).collect();
    if abs.is_empty() {
        return -70.0;
    }
    let mean = abs.iter().sum::<f64>() / abs.len() as f64;
    let rel_gate = lufs(mean) - 10.0;
    let gated: Vec<f64> = abs.into_iter().filter(|&e| lufs(e) > rel_gate).collect();
    if gated.is_empty() {
        return -70.0;
    }
    lufs(gated.iter().sum::<f64>() / gated.len() as f64)
}

/// Peak with optional oversampling (ratio 1 = sample peak, up to 16).
pub fn true_peak(a: &Audio, ratio: u32) -> f32 {
    let ratio = ratio.clamp(1, 16) as usize;
    if ratio <= 1 {
        return a.peak();
    }
    let ch = a.channels.max(1);
    let frames = a.frames();
    let taps = 16isize;
    let mut peak = a.peak();
    for c in 0..ch {
        for i in 0..frames {
            for p in 1..ratio {
                let t = p as f64 / ratio as f64;
                let mut acc = 0f64;
                for k in -taps + 1..=taps {
                    let idx = i as isize + k;
                    if idx < 0 || idx >= frames as isize {
                        continue;
                    }
                    let x = k as f64 - t;
                    let sinc = if x.abs() < 1e-9 { 1.0 } else { (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x) };
                    let w = 0.5 + 0.5 * (std::f64::consts::PI * x / taps as f64).cos();
                    acc += a.data[idx as usize * ch + c] as f64 * sinc * w;
                }
                peak = peak.max(acc.abs() as f32);
            }
        }
    }
    peak
}

/// Gain in dB to reach `target` LUFS, limited to `max_gain` and (unless disabled) to avoid clipping.
pub fn replaygain(loudness: f64, peak: f32, target: f64, max_gain: f64, clip_prevention: bool) -> f64 {
    let mut gain = (target - loudness).min(max_gain);
    if clip_prevention && peak > 0.0 {
        let max_no_clip = -20.0 * (peak as f64).log10();
        if gain > max_no_clip {
            gain = max_no_clip;
        }
    }
    gain
}
