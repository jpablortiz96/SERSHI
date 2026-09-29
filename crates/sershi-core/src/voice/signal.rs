//! Audio normalisation: pure, deterministic signal processing shared by
//! capture (device format → 16 kHz mono for recognition) and playback
//! (synthesized speech → the output device's rate).
//!
//! Microphones rarely deliver the recogniser's format: Windows shared-mode
//! devices typically run at 44.1 or 48 kHz, stereo, 32-bit float. SERSHI
//! keeps a push-to-talk utterance (bounded to [`super::endpoint`]'s maximum)
//! in memory at the device rate and resamples it once when capture ends.
//!
//! The resampler is a windowed-sinc (Blackman) interpolator with a
//! precomputed kernel table: band-limited, so downsampling 48 → 16 kHz does
//! not alias, and fast enough for a 30 s utterance in tens of milliseconds.

/// Sample rate the speech recogniser expects.
pub const RECOGNITION_RATE: u32 = 16_000;

/// Averages interleaved channels into mono.
pub fn downmix(interleaved: &[f32], channels: u16, out: &mut Vec<f32>) {
    let channels = usize::from(channels.max(1));
    if channels == 1 {
        out.extend_from_slice(interleaved);
        return;
    }
    let scale = 1.0 / channels as f32;
    out.extend(
        interleaved
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() * scale),
    );
}

/// Converts signed 16-bit PCM to `f32` in −1…1.
pub fn i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| f32::from(s) / 32_768.0).collect()
}

/// Kernel zero-crossings on each side of the centre at the output's cutoff.
const HALF_ZERO_CROSSINGS: f64 = 16.0;
/// Kernel table resolution (entries per input sample).
const TABLE_STEPS: f64 = 64.0;
/// Cutoff as a fraction of the lower Nyquist frequency (headroom for the
/// transition band).
const CUTOFF: f64 = 0.94;

/// Resamples a mono signal from `from` Hz to `to` Hz.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || from == 0 || to == 0 || input.is_empty() {
        return input.to_vec();
    }
    let ratio = f64::from(to) / f64::from(from);
    // Normalised cutoff in cycles per *input* sample ×2 (1.0 = input Nyquist).
    let cutoff = ratio.min(1.0) * CUTOFF;
    // Half-width of the kernel, in input samples.
    let half_width = HALF_ZERO_CROSSINGS / cutoff;
    let table = kernel_table(cutoff, half_width);

    let out_len = ((input.len() as f64) * ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    let last = input.len() as isize - 1;
    for n in 0..out_len {
        let centre = n as f64 / ratio;
        let first = (centre - half_width).ceil() as isize;
        let end = (centre + half_width).floor() as isize;
        let mut acc = 0.0f64;
        let mut weight = 0.0f64;
        for k in first.max(0)..=end.min(last) {
            let w = lookup(&table, (centre - k as f64).abs());
            acc += w * f64::from(input[k as usize]);
            weight += w;
        }
        // Normalising by the summed weights keeps DC gain exactly 1 at the
        // edges, where the kernel is truncated.
        out.push(if weight > 1e-9 {
            (acc / weight) as f32
        } else {
            0.0
        });
    }
    out
}

fn kernel_table(cutoff: f64, half_width: f64) -> Vec<f64> {
    let len = (half_width * TABLE_STEPS).ceil() as usize + 2;
    (0..len)
        .map(|i| {
            let x = i as f64 / TABLE_STEPS;
            if x >= half_width {
                return 0.0;
            }
            let sinc = if x == 0.0 {
                1.0
            } else {
                let a = std::f64::consts::PI * cutoff * x;
                a.sin() / a
            };
            // Blackman window over [-half_width, half_width].
            let t = 0.5 + 0.5 * (x / half_width);
            let window = 0.42 - 0.5 * (2.0 * std::f64::consts::PI * t).cos()
                + 0.08 * (4.0 * std::f64::consts::PI * t).cos();
            sinc * window
        })
        .collect()
}

fn lookup(table: &[f64], distance: f64) -> f64 {
    let pos = distance * TABLE_STEPS;
    let i = pos.floor() as usize;
    match (table.get(i), table.get(i + 1)) {
        (Some(a), Some(b)) => {
            let frac = pos - i as f64;
            a + (b - a) * frac
        }
        (Some(a), None) => *a,
        _ => 0.0,
    }
}

/// Root-mean-square level of a block, in dBFS (−120 for silence).
pub fn rms_dbfs(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -120.0;
    }
    let power = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    if power <= 1e-12 {
        -120.0
    } else {
        (10.0 * power.log10()).max(-120.0)
    }
}

/// Turns block levels into a bounded 0–1 value for visuals: quiet rooms sit
/// near 0, normal speech lands around 0.4–0.8, and nothing ever exceeds 1.
/// Fast attack, slower release, so the companion follows speech without
/// flicker. Presentation only — never used for decisions.
#[derive(Debug, Clone)]
pub struct LevelMeter {
    value: f32,
}

/// dBFS mapped to 0.
const METER_FLOOR_DB: f32 = -60.0;
/// dBFS mapped to 1.
const METER_CEILING_DB: f32 = -12.0;

impl Default for LevelMeter {
    fn default() -> Self {
        Self::new()
    }
}

impl LevelMeter {
    pub fn new() -> Self {
        Self { value: 0.0 }
    }

    /// Feeds one block and returns the smoothed level.
    pub fn push(&mut self, block: &[f32]) -> f32 {
        let db = rms_dbfs(block);
        let target = ((db - METER_FLOOR_DB) / (METER_CEILING_DB - METER_FLOOR_DB)).clamp(0.0, 1.0);
        let rate = if target > self.value { 0.6 } else { 0.15 };
        self.value += (target - self.value) * rate;
        self.value = self.value.clamp(0.0, 1.0);
        self.value
    }

    pub fn value(&self) -> f32 {
        self.value
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    //! Deterministic, generated test audio (no recordings, nothing licensed).

    /// A sine tone at `hz`, `amplitude` peak, `ms` long.
    pub fn tone(rate: u32, hz: f32, amplitude: f32, ms: u32) -> Vec<f32> {
        let n = (rate as u64 * u64::from(ms) / 1000) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / rate as f32;
                amplitude * (2.0 * std::f32::consts::PI * hz * t).sin()
            })
            .collect()
    }

    /// Low-level pseudo-random noise (deterministic LCG), like a quiet room.
    pub fn noise(rate: u32, amplitude: f32, ms: u32, seed: u32) -> Vec<f32> {
        let n = (rate as u64 * u64::from(ms) / 1000) as usize;
        let mut state = seed.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
        (0..n)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let unit = (state >> 8) as f32 / (1u32 << 24) as f32;
                amplitude * (unit * 2.0 - 1.0)
            })
            .collect()
    }

    /// Speech-like bursts: a voiced tone modulated at a syllable rate.
    pub fn speech(rate: u32, amplitude: f32, ms: u32) -> Vec<f32> {
        tone(rate, 180.0, 1.0, ms)
            .into_iter()
            .enumerate()
            .map(|(i, s)| {
                let t = i as f32 / rate as f32;
                let syllables = 0.6 + 0.4 * (2.0 * std::f32::consts::PI * 4.0 * t).sin();
                amplitude * s * syllables
            })
            .collect()
    }

    pub fn concat(parts: &[Vec<f32>]) -> Vec<f32> {
        parts.iter().flatten().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    /// Amplitude of `hz` in `signal` (single-bin DFT, normalised).
    fn amplitude_at(signal: &[f32], rate: u32, hz: f32) -> f32 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, s) in signal.iter().enumerate() {
            let phase = 2.0 * std::f64::consts::PI * f64::from(hz) * i as f64 / f64::from(rate);
            re += f64::from(*s) * phase.cos();
            im += f64::from(*s) * phase.sin();
        }
        (2.0 * (re * re + im * im).sqrt() / signal.len() as f64) as f32
    }

    #[test]
    fn downmix_averages_channels() {
        let mut out = Vec::new();
        downmix(&[1.0, 0.0, 0.5, 0.5, -1.0, 1.0], 2, &mut out);
        assert_eq!(out, [0.5, 0.5, 0.0]);
        let mut mono = Vec::new();
        downmix(&[0.1, 0.2], 1, &mut mono);
        assert_eq!(mono, [0.1, 0.2]);
    }

    #[test]
    fn pcm16_converts_to_unit_range() {
        assert_eq!(i16_to_f32(&[0, 16_384, -32_768]), [0.0, 0.5, -1.0]);
    }

    #[test]
    fn resampling_48k_to_16k_keeps_duration_and_speech_band() {
        let input = tone(48_000, 440.0, 0.5, 1_000);
        let out = resample(&input, 48_000, RECOGNITION_RATE);
        assert_eq!(out.len(), 16_000);
        let a = amplitude_at(&out[1_000..15_000], 16_000, 440.0);
        assert!((a - 0.5).abs() < 0.02, "440 Hz amplitude {a}");
    }

    #[test]
    fn resampling_rejects_content_above_the_new_nyquist() {
        // 12 kHz cannot exist at 16 kHz; it must be filtered, not folded
        // back to 4 kHz where it would sound like speech energy.
        let input = tone(48_000, 12_000.0, 0.5, 500);
        let out = resample(&input, 48_000, 16_000);
        let aliased = amplitude_at(&out[500..7_500], 16_000, 4_000.0);
        let rms = rms_dbfs(&out[500..7_500]);
        assert!(aliased < 0.01, "aliased {aliased}");
        assert!(rms < -40.0, "residual {rms} dBFS");
    }

    #[test]
    fn resampling_handles_uncommon_rates_and_upsampling() {
        let input = tone(44_100, 300.0, 0.4, 500);
        let out = resample(&input, 44_100, 16_000);
        assert_eq!(out.len(), 8_000);
        assert!((amplitude_at(&out[500..7_500], 16_000, 300.0) - 0.4).abs() < 0.02);

        let speech = tone(16_000, 300.0, 0.4, 500);
        let up = resample(&speech, 16_000, 48_000);
        assert_eq!(up.len(), 24_000);
        assert!((amplitude_at(&up[1_000..23_000], 48_000, 300.0) - 0.4).abs() < 0.02);
    }

    #[test]
    fn resampling_edge_cases() {
        assert!(resample(&[], 48_000, 16_000).is_empty());
        assert_eq!(resample(&[0.25; 10], 16_000, 16_000), [0.25; 10]);
        // A constant signal stays constant (DC gain 1, including edges).
        for s in resample(&[0.3; 4_800], 48_000, 16_000) {
            assert!((s - 0.3).abs() < 1e-3, "{s}");
        }
    }

    #[test]
    fn level_meter_is_bounded_and_follows_speech() {
        let mut meter = LevelMeter::new();
        assert_eq!(meter.push(&[0.0; 320]), 0.0);
        let loud = tone(16_000, 200.0, 1.0, 20);
        let mut level = 0.0;
        for _ in 0..10 {
            level = meter.push(&loud);
        }
        assert!(level > 0.9 && level <= 1.0, "{level}");
        let quiet = noise(16_000, 0.001, 20, 1);
        for _ in 0..30 {
            level = meter.push(&quiet);
        }
        assert!(level < 0.1, "{level}");
        assert_eq!(rms_dbfs(&[]), -120.0);
    }
}
