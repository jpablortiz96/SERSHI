//! Speech endpointing for push-to-talk: decides when an utterance has ended
//! so the user does not have to click again, and when nothing was said.
//!
//! **This is a user-experience feature, not a security feature.** It never
//! decides whether anything runs: a transcript still goes through the
//! command pipeline and policy, and the microphone only ever opens because
//! the user asked.
//!
//! Approach: a conservative energy detector with an adaptive noise floor
//! over 20 ms frames. It works at any sample rate, needs no model, and is
//! fully deterministic, so it can be tested with generated audio. A neural
//! VAD (e.g. Silero) can replace it behind the same interface later; see
//! docs/VOICE.md.

use super::signal::rms_dbfs;

/// Tunables. Defaults were chosen for a quiet-to-normal room and are
/// refined by the Gate 3A physical tests (docs/VOICE.md).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EndpointConfig {
    pub frame_ms: u32,
    /// Speech must last at least this long in total to count as speech.
    pub min_speech_ms: u32,
    /// Silence after speech that ends the utterance (pauses between words
    /// are much shorter).
    pub trailing_silence_ms: u32,
    /// Hard cap on one push-to-talk capture.
    pub max_capture_ms: u32,
    /// Stop if nothing resembling speech arrives this long after start.
    pub no_speech_timeout_ms: u32,
    /// How far above the noise floor a frame must be to count as speech.
    pub threshold_over_noise_db: f32,
    /// Frames quieter than this are never speech, whatever the noise floor.
    pub absolute_floor_dbfs: f32,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            frame_ms: 20,
            min_speech_ms: 200,
            trailing_silence_ms: 1_000,
            max_capture_ms: 30_000,
            no_speech_timeout_ms: 8_000,
            threshold_over_noise_db: 10.0,
            absolute_floor_dbfs: -50.0,
        }
    }
}

/// Frames used to seed the noise floor (100 ms at the default frame size).
const CALIBRATION_FRAMES: u32 = 5;
/// The seeded noise floor never starts above this (a loud room adapts up).
const MAX_SEED_NOISE_DBFS: f32 = -45.0;

/// What the detector concluded after the latest audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    /// Keep listening.
    Continue,
    /// Speech was heard and has been followed by sustained silence.
    SpeechEnded,
    /// The maximum capture length was reached; transcribe what was heard.
    MaxDuration,
    /// Nothing resembling speech arrived in time.
    NoSpeech,
}

#[derive(Debug, Clone)]
pub struct EndpointDetector {
    config: EndpointConfig,
    frame_len: usize,
    pending: Vec<f32>,
    frames: u32,
    noise_db: Option<f32>,
    speech_frames: u32,
    trailing_silence: u32,
    heard_speech: bool,
    any_signal: bool,
    finished: Option<Endpoint>,
}

impl EndpointDetector {
    pub fn new(sample_rate: u32, config: EndpointConfig) -> Self {
        let frame_len = ((u64::from(sample_rate) * u64::from(config.frame_ms)) / 1000).max(1);
        Self {
            config,
            frame_len: usize::try_from(frame_len).unwrap_or(320),
            pending: Vec::new(),
            frames: 0,
            noise_db: None,
            speech_frames: 0,
            trailing_silence: 0,
            heard_speech: false,
            any_signal: false,
            finished: None,
        }
    }

    /// Feeds mono samples. Once a terminal endpoint is reached it is
    /// returned for every later call.
    pub fn push(&mut self, samples: &[f32]) -> Endpoint {
        if let Some(done) = self.finished {
            return done;
        }
        self.pending.extend_from_slice(samples);
        let mut consumed = 0;
        while self.pending.len() - consumed >= self.frame_len {
            let frame = &self.pending[consumed..consumed + self.frame_len];
            consumed += self.frame_len;
            if frame.iter().any(|s| *s != 0.0) {
                self.any_signal = true;
            }
            let db = rms_dbfs(frame);
            if let Some(end) = self.frame(db) {
                self.finished = Some(end);
                self.pending.clear();
                return end;
            }
        }
        self.pending.drain(..consumed);
        Endpoint::Continue
    }

    fn frame(&mut self, db: f32) -> Option<Endpoint> {
        self.frames += 1;
        let c = &self.config;
        // People take a few hundred milliseconds to start talking after they
        // click; the quietest of the first frames seeds the noise floor.
        // Capped, so someone who starts talking immediately is still heard.
        if self.frames <= CALIBRATION_FRAMES {
            let seed = db.min(MAX_SEED_NOISE_DBFS);
            self.noise_db = Some(self.noise_db.map_or(seed, |n| n.min(seed)));
            return None;
        }
        let noise = self.noise_db.unwrap_or(db);
        let threshold = (noise + c.threshold_over_noise_db).max(c.absolute_floor_dbfs);
        let is_speech = db > threshold;

        // The floor falls quickly (a door closed), rises slowly in pauses and
        // barely at all during speech, so it never learns the voice as noise.
        let rate = if db < noise {
            0.3
        } else if is_speech {
            0.002
        } else {
            0.05
        };
        self.noise_db = Some(noise + (db - noise) * rate);

        if is_speech {
            self.speech_frames += 1;
            self.trailing_silence = 0;
            if self.ms(self.speech_frames) >= c.min_speech_ms {
                self.heard_speech = true;
            }
        } else {
            self.trailing_silence += 1;
        }

        let elapsed = self.ms(self.frames);
        if elapsed >= c.max_capture_ms {
            return Some(if self.heard_speech {
                Endpoint::MaxDuration
            } else {
                Endpoint::NoSpeech
            });
        }
        if self.heard_speech && self.ms(self.trailing_silence) >= c.trailing_silence_ms {
            return Some(Endpoint::SpeechEnded);
        }
        if !self.heard_speech && elapsed >= c.no_speech_timeout_ms {
            return Some(Endpoint::NoSpeech);
        }
        None
    }

    fn ms(&self, frames: u32) -> u32 {
        frames.saturating_mul(self.config.frame_ms)
    }

    /// Whether enough speech was heard to be worth transcribing.
    pub fn heard_speech(&self) -> bool {
        self.heard_speech
    }

    /// True while every sample so far was exactly zero. Windows delivers
    /// digital silence (rather than an error) to desktop apps when
    /// microphone access is turned off in privacy settings.
    pub fn digital_silence(&self) -> bool {
        !self.any_signal
    }

    /// Captured length in milliseconds (whole frames).
    pub fn elapsed_ms(&self) -> u32 {
        self.ms(self.frames)
    }
}

#[cfg(test)]
mod tests {
    use super::super::signal::fixtures::*;
    use super::*;

    const RATE: u32 = 48_000;

    fn run(detector: &mut EndpointDetector, audio: &[f32]) -> Endpoint {
        // Feed in device-sized callbacks (10 ms).
        let mut last = Endpoint::Continue;
        for chunk in audio.chunks(480) {
            last = detector.push(chunk);
            if last != Endpoint::Continue {
                break;
            }
        }
        last
    }

    fn detector() -> EndpointDetector {
        EndpointDetector::new(RATE, EndpointConfig::default())
    }

    #[test]
    fn speech_followed_by_silence_ends_the_utterance() {
        let audio = concat(&[
            noise(RATE, 0.002, 400, 1),
            speech(RATE, 0.3, 1_200),
            noise(RATE, 0.002, 1_500, 2),
        ]);
        let mut d = detector();
        assert_eq!(run(&mut d, &audio), Endpoint::SpeechEnded);
        assert!(d.heard_speech());
        // Ended ~1 s after the speech, not at the end of the fixture.
        assert!(
            d.elapsed_ms() >= 2_500 && d.elapsed_ms() <= 2_800,
            "{}",
            d.elapsed_ms()
        );
    }

    #[test]
    fn short_pauses_between_words_do_not_end_the_utterance() {
        let audio = concat(&[
            speech(RATE, 0.3, 600),
            noise(RATE, 0.002, 400, 3),
            speech(RATE, 0.3, 600),
            noise(RATE, 0.002, 500, 4),
            speech(RATE, 0.3, 400),
        ]);
        let mut d = detector();
        assert_eq!(run(&mut d, &audio), Endpoint::Continue);
        assert!(d.heard_speech());
    }

    #[test]
    fn silence_and_room_noise_are_not_speech() {
        let mut d = detector();
        assert_eq!(
            run(&mut d, &noise(RATE, 0.003, 9_000, 5)),
            Endpoint::NoSpeech
        );
        assert!(!d.heard_speech());
        assert!(!d.digital_silence());
    }

    #[test]
    fn a_click_is_not_speech() {
        let audio = concat(&[
            noise(RATE, 0.002, 500, 6),
            speech(RATE, 0.5, 60),
            noise(RATE, 0.002, 2_000, 7),
        ]);
        let mut d = detector();
        run(&mut d, &audio);
        assert!(!d.heard_speech());
    }

    #[test]
    fn endless_speech_stops_at_the_maximum() {
        let config = EndpointConfig {
            max_capture_ms: 3_000,
            ..EndpointConfig::default()
        };
        let mut d = EndpointDetector::new(RATE, config);
        assert_eq!(
            run(&mut d, &speech(RATE, 0.3, 5_000)),
            Endpoint::MaxDuration
        );
        assert_eq!(d.elapsed_ms(), 3_000);
        // Terminal: later audio does not change the decision.
        assert_eq!(d.push(&[0.0; 480]), Endpoint::MaxDuration);
    }

    #[test]
    fn digital_silence_is_detected() {
        let mut d = detector();
        run(&mut d, &vec![0.0; RATE as usize]);
        assert!(d.digital_silence());
        assert!(!d.heard_speech());
    }

    #[test]
    fn works_at_the_recognition_rate_too() {
        let audio = concat(&[speech(16_000, 0.2, 800), noise(16_000, 0.001, 1_200, 8)]);
        let mut d = EndpointDetector::new(16_000, EndpointConfig::default());
        assert_eq!(run(&mut d, &audio), Endpoint::SpeechEnded);
    }
}
