//! Minimal RIFF/WAVE reader for synthesized speech held in memory.
//!
//! Windows speech synthesis returns a WAV stream (16-bit PCM). Only what
//! SERSHI needs is supported: PCM 16-bit, any channel count (mixed to
//! mono). Anything else is rejected rather than guessed.

use sershi_core::voice::ports::SpeechAudio;
use sershi_core::voice::signal::downmix;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavError {
    NotWav,
    Unsupported,
    Truncated,
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(i)?,
        *b.get(i + 1)?,
        *b.get(i + 2)?,
        *b.get(i + 3)?,
    ]))
}

/// Decodes a PCM16 WAV file into mono `f32`.
pub fn decode(bytes: &[u8]) -> Result<SpeechAudio, WavError> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(WavError::NotWav);
    }
    let mut pos = 12;
    let mut format: Option<(u16, u32)> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32_at(bytes, pos + 4).ok_or(WavError::Truncated)? as usize;
        let body = pos + 8;
        match id {
            b"fmt " => {
                let tag = u16_at(bytes, body).ok_or(WavError::Truncated)?;
                let channels = u16_at(bytes, body + 2).ok_or(WavError::Truncated)?;
                let rate = u32_at(bytes, body + 4).ok_or(WavError::Truncated)?;
                let bits = u16_at(bytes, body + 14).ok_or(WavError::Truncated)?;
                // 1 = PCM. (WAVE_FORMAT_EXTENSIBLE is not produced here.)
                if tag != 1 || bits != 16 || channels == 0 || rate == 0 {
                    return Err(WavError::Unsupported);
                }
                format = Some((channels, rate));
            }
            b"data" => {
                let (channels, rate) = format.ok_or(WavError::Unsupported)?;
                let end = body.saturating_add(size).min(bytes.len());
                let (pairs, _) = bytes[body..end].as_chunks::<2>();
                let pcm: Vec<f32> = pairs
                    .iter()
                    .map(|c| f32::from(i16::from_le_bytes(*c)) / 32_768.0)
                    .collect();
                let mut samples = Vec::with_capacity(pcm.len() / usize::from(channels));
                downmix(&pcm, channels, &mut samples);
                return Ok(SpeechAudio {
                    samples,
                    sample_rate: rate,
                });
            }
            _ => {}
        }
        // Chunks are word-aligned.
        pos = body.saturating_add(size + (size & 1));
    }
    Err(WavError::Truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(channels: u16, rate: u32, samples: &[i16], extra_fmt: bool) -> Vec<u8> {
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&rate.to_le_bytes());
        fmt.extend_from_slice(&(rate * 2 * u32::from(channels)).to_le_bytes());
        fmt.extend_from_slice(&(2 * channels).to_le_bytes());
        fmt.extend_from_slice(&16u16.to_le_bytes());
        if extra_fmt {
            // WAVEFORMATEX cbSize, as Windows speech synthesis writes it.
            fmt.extend_from_slice(&0u16.to_le_bytes());
        }
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&((4 + 8 + fmt.len() + 8 + data.len()) as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        out.extend_from_slice(&fmt);
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        out
    }

    #[test]
    fn decodes_windows_speech_output() {
        let audio = decode(&wav(1, 16_000, &[0, 16_384, -32_768], true)).unwrap();
        assert_eq!(audio.sample_rate, 16_000);
        assert_eq!(audio.samples, [0.0, 0.5, -1.0]);
    }

    #[test]
    fn mixes_stereo_to_mono() {
        let audio = decode(&wav(2, 22_050, &[16_384, 0, -16_384, -16_384], false)).unwrap();
        assert_eq!(audio.samples, [0.25, -0.5]);
    }

    #[test]
    fn rejects_other_formats_and_garbage() {
        assert_eq!(decode(b"not a wav file at all"), Err(WavError::NotWav));
        let mut float = wav(1, 16_000, &[1, 2], false);
        float[20] = 3; // WAVE_FORMAT_IEEE_FLOAT
        assert_eq!(decode(&float), Err(WavError::Unsupported));
        let full = wav(1, 16_000, &[1, 2], false);
        assert_eq!(decode(&full[..30]), Err(WavError::Truncated));
    }
}
