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
    /// Silence that ends a short utterance ("Abre Chrome"). Pauses between
    /// words are much shorter.
    pub short_silence_ms: u32,
    /// Silence that ends a longer request, which tolerates more thinking
    /// pauses.
    pub long_silence_ms: u32,
    /// Utterances with less speech than this use `short_silence_ms`.
    pub short_utterance_ms: u32,
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
            // Gate 3B: was a fixed 1 000 ms. Short commands end after
            // 600 ms, longer requests after 900 ms (tuned on physical speech;
            // docs/VOICE.md#endpointing).
            short_silence_ms: 600,
            long_silence_ms: 900,
            short_utterance_ms: 1_500,
            max_capture_ms: 30_000,
            no_speech_timeout_ms: 8_000,
            threshold_over_noise_db: 10.0,
            absolute_floor_dbfs: -50.0,
        }
    }
}

impl EndpointConfig {
    /// One fixed silence for every utterance (developer tuning).
    pub fn with_silence(self, ms: u32) -> Self {
        Self {
            short_silence_ms: ms,
            long_silence_ms: ms,
            ..self
        }
    }

    /// The silence that ends an utterance with `speech_ms` of speech.
    pub fn silence_for(&self, speech_ms: u32) -> u32 {
        if speech_ms < self.short_utterance_ms {
            self.short_silence_ms
        } else {
            self.long_silence_ms
        }
    }
}

/// Frames used to seed the noise floor (100 ms at the default frame size).
const CALIBRATION_FRAMES: u32 = 5;
/// The seeded noise floor never starts above this (a loud room adapts up).
const MAX_SEED_NOISE_DBFS: f32 = -45.0;
/// Speech onset (Gate 4.1.1): `min_speech_ms` of speech within this
/// sliding window — at least half of it voiced. Keyboard clicks, a cough or
/// a bump over a long wait never add up to "speech" and never start
/// recognition; a spoken word is mostly voiced and qualifies at once.
const ONSET_WINDOW_MS: u32 = 400;

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
    /// Before speech is heard: the speech frames of the last
    /// `ONSET_WINDOW_MS`.
    onset: std::collections::VecDeque<u32>,
    trailing_silence: u32,
    heard_speech: bool,
    any_signal: bool,
    first_speech_frame: Option<u32>,
    last_speech_frame: Option<u32>,
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
            onset: std::collections::VecDeque::new(),
            trailing_silence: 0,
            heard_speech: false,
            any_signal: false,
            first_speech_frame: None,
            last_speech_frame: None,
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

        if !self.heard_speech {
            if is_speech {
                self.trailing_silence = 0;
                self.onset.push_back(self.frames);
            } else {
                self.trailing_silence += 1;
            }
            let window = (ONSET_WINDOW_MS / c.frame_ms).max(1);
            while self
                .onset
                .front()
                .is_some_and(|f| self.frames.saturating_sub(*f) >= window)
            {
                self.onset.pop_front();
            }
            let voiced = u32::try_from(self.onset.len()).unwrap_or(u32::MAX);
            if self.ms(voiced) >= c.min_speech_ms {
                // Speech starts at its first voiced frame in the window.
                self.heard_speech = true;
                self.speech_frames = voiced;
                self.first_speech_frame = self.onset.front().copied();
                self.last_speech_frame = self.onset.back().copied();
                self.onset.clear();
            }
        } else if is_speech {
            self.speech_frames += 1;
            self.trailing_silence = 0;
            self.last_speech_frame = Some(self.frames);
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
        let needed = c.silence_for(self.ms(self.speech_frames));
        if self.heard_speech && self.ms(self.trailing_silence) >= needed {
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

    /// Silence since the last speech frame, once speech was heard (for the
    /// early decode that overlaps the endpoint wait).
    pub fn pause_ms(&self) -> Option<u32> {
        self.heard_speech.then(|| self.ms(self.trailing_silence))
    }

    /// Start of the first speech frame, in ms from the start of capture.
    pub fn speech_started_ms(&self) -> Option<u32> {
        self.first_speech_frame.map(|f| self.ms(f - 1))
    }

    /// End of the last speech frame, in ms from the start of capture.
    pub fn speech_ended_ms(&self) -> Option<u32> {
        self.last_speech_frame.map(|f| self.ms(f))
    }

    /// Samples up to the end of the last speech frame (for deciding whether
    /// an early decode covered everything that was said).
    pub fn last_speech_sample(&self) -> usize {
        self.last_speech_frame
            .map_or(0, |f| f as usize * self.frame_len)
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

    /// Keyboard-like transients: 15 ms clicks every 150 ms.
    fn typing(ms: u32) -> Vec<f32> {
        let mut out = Vec::new();
        let mut t = 0;
        while t < ms {
            out.extend(speech(RATE, 0.3, 15));
            out.extend(noise(RATE, 0.002, 135, t + 3));
            t += 150;
        }
        out
    }

    #[test]
    fn scattered_transients_over_a_long_wait_are_never_speech() {
        // Gate 4.1.1: a voice session waits up to 25 s; clicks, a cough
        // and bumps must not add up to "speech" and start recognition.
        let config = EndpointConfig {
            no_speech_timeout_ms: 25_000,
            ..EndpointConfig::default()
        };
        let mut parts = vec![noise(RATE, 0.003, 500, 7)];
        for i in 0..12 {
            parts.push(typing(1_500));
            parts.push(speech(RATE, 0.3, 120)); // a cough or a bump
            parts.push(noise(RATE, 0.003, 600, 20 + i));
        }
        parts.push(noise(RATE, 0.003, 4_000, 99));
        let audio = concat(&parts);
        let mut d = EndpointDetector::new(RATE, config);
        assert_eq!(run(&mut d, &audio), Endpoint::NoSpeech);
        assert!(!d.heard_speech());
    }

    #[test]
    fn steady_fan_noise_is_never_speech() {
        let config = EndpointConfig {
            no_speech_timeout_ms: 10_000,
            ..EndpointConfig::default()
        };
        let audio = noise(RATE, 0.02, 11_000, 5);
        let mut d = EndpointDetector::new(RATE, config);
        assert_eq!(run(&mut d, &audio), Endpoint::NoSpeech);
    }

    #[test]
    fn speech_after_a_long_wait_is_heard_promptly_and_from_its_start() {
        let config = EndpointConfig {
            no_speech_timeout_ms: 25_000,
            ..EndpointConfig::default()
        };
        let audio = concat(&[
            noise(RATE, 0.003, 300, 1),
            typing(3_000),
            noise(RATE, 0.003, 2_000, 2),
            speech(RATE, 0.3, 1_200),
            noise(RATE, 0.003, 1_500, 3),
        ]);
        let mut d = EndpointDetector::new(RATE, config);
        assert_eq!(run(&mut d, &audio), Endpoint::SpeechEnded);
        // The onset is the speech, not the typing before it (so recognition
        // can skip what came before, with a short pre-roll).
        let start = d.speech_started_ms().expect("speech");
        assert!((5_280..=5_340).contains(&start), "onset at {start} ms");
    }

    #[test]
    fn a_short_command_ends_after_a_short_silence() {
        let audio = concat(&[
            noise(RATE, 0.002, 400, 1),
            speech(RATE, 0.3, 1_000),
            noise(RATE, 0.002, 1_500, 2),
        ]);
        let mut d = detector();
        assert_eq!(run(&mut d, &audio), Endpoint::SpeechEnded);
        assert!(d.heard_speech());
        // ~600 ms after the last word, not at the end of the fixture.
        let ended = d.speech_ended_ms().unwrap();
        let waited = d.elapsed_ms() - ended;
        assert!((600..=660).contains(&waited), "waited {waited} ms");
        assert!(d.speech_started_ms().unwrap() >= 400 && d.speech_started_ms().unwrap() <= 480);
    }

    #[test]
    fn a_longer_request_tolerates_a_longer_pause() {
        let audio = concat(&[
            speech(RATE, 0.3, 2_400),
            noise(RATE, 0.002, 750, 3),
            speech(RATE, 0.3, 600),
            noise(RATE, 0.002, 1_500, 4),
        ]);
        let mut d = detector();
        assert_eq!(run(&mut d, &audio), Endpoint::SpeechEnded);
        // The 750 ms pause did not end it; the final 900 ms silence did.
        let waited = d.elapsed_ms() - d.speech_ended_ms().unwrap();
        assert!((900..=960).contains(&waited), "waited {waited} ms");
        assert!(d.speech_ended_ms().unwrap() > 3_500);
    }

    #[test]
    fn silence_thresholds_are_adaptive_and_overridable() {
        let c = EndpointConfig::default();
        assert_eq!(c.silence_for(800), 600);
        assert_eq!(c.silence_for(2_000), 900);
        let fixed = c.with_silence(450);
        assert_eq!(
            (fixed.silence_for(800), fixed.silence_for(5_000)),
            (450, 450)
        );
    }

    #[test]
    fn the_pause_is_reported_only_after_speech() {
        let mut d = detector();
        run(&mut d, &noise(RATE, 0.002, 500, 5));
        assert_eq!(d.pause_ms(), None);
        run(&mut d, &speech(RATE, 0.3, 600));
        run(&mut d, &noise(RATE, 0.002, 300, 6));
        let pause = d.pause_ms().unwrap();
        assert!((280..=320).contains(&pause), "{pause}");
        assert!(d.last_speech_sample() > 0);
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
