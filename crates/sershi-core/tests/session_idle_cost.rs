//! Gate 4.1 resource check: what a voice session costs while it waits for
//! the user to speak.
//!
//! While listening, SERSHI only measures audio energy (the endpoint
//! detector); recognition never runs over silence (an early decode needs
//! speech first). This times the detector over a whole idle timeout of quiet
//! room noise at 48 kHz and reports it as a share of real time.
//!
//! `cargo test -p sershi-core --release --test session_idle_cost -- --nocapture`

use std::time::Instant;

use sershi_core::voice::endpoint::{Endpoint, EndpointConfig, EndpointDetector};
use sershi_core::voice::session::IDLE_TIMEOUT_MS;

#[test]
fn listening_for_a_whole_idle_timeout_costs_a_tiny_share_of_one_core() {
    const RATE: u32 = 48_000;
    let config = EndpointConfig {
        no_speech_timeout_ms: IDLE_TIMEOUT_MS,
        ..EndpointConfig::default()
    };
    let mut detector = EndpointDetector::new(RATE, config);
    // Quiet room noise around -60 dBFS, in 10 ms device buffers.
    let mut seed = 0x2545_f491_u32;
    let mut noise = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32 - 0.5) * 0.002
    };
    let chunk = (RATE / 100) as usize;
    let chunks = IDLE_TIMEOUT_MS as usize / 10 + 50;
    let buffers: Vec<Vec<f32>> = (0..chunks)
        .map(|_| (0..chunk).map(|_| noise()).collect())
        .collect();

    let started = Instant::now();
    let mut ended = None;
    for (i, buffer) in buffers.iter().enumerate() {
        match detector.push(buffer) {
            Endpoint::Continue => {}
            other => {
                ended = Some((i, other));
                break;
            }
        }
    }
    let spent = started.elapsed();

    let (at, endpoint) = ended.expect("the idle timeout ends listening");
    assert_eq!(endpoint, Endpoint::NoSpeech, "silence is never speech");
    let audio_ms = (at as u64 + 1) * 10;
    assert!(audio_ms >= u64::from(IDLE_TIMEOUT_MS));
    let share = spent.as_secs_f64() * 1000.0 / audio_ms as f64;
    println!(
        "idle listening: {audio_ms} ms of audio analysed in {:.2} ms ({:.4} % of one core)",
        spent.as_secs_f64() * 1000.0,
        share * 100.0
    );
    // Generous bound for slow CI machines and debug builds: under 5 % of
    // one core (measured far below that; docs/VOICE.md#voice-session).
    assert!(
        share < 0.05,
        "idle listening used {:.2} % of a core",
        share * 100.0
    );
}
